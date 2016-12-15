
use reader_writer::{CStr, Lazy, LazySized, Reader, Readable, RoArray, Writable};
use reader_writer::typenum::*;
use reader_writer::generic_array::GenericArray;

use std::fmt;
use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ascii::AsciiExt;

use ::pak::Pak;

// Based on http://hitmen.c02.at/files/yagcd/yagcd/chap13.html

pub const GC_DISC_LENGTH: usize = 1_459_978_240;

pub struct GcDisc<'a>
{
    pub header: GcDiscHeader,
    header_info: Lazy<'a, Box<GenericArray<u8, U8192>>>,
    apploader: GcDiscApploader<'a>,
    pub file_system_table: FileSystemTable<'a>,
}

impl<'a> Readable<'a> for GcDisc<'a>
{
    type Args = ();
    fn read(mut reader: Reader<'a>, (): ()) -> (GcDisc<'a>, Reader<'a>)
    {
        let start = reader.clone();
        let header: GcDiscHeader = reader.read(());
        let header_info = reader.read(());
        let apploader = reader.read(());
        let fst = reader.read((start, header.fst_offset as usize, header.fst_length as usize));

        let gc_disc = GcDisc {
            header: header,
            header_info: header_info,
            apploader: apploader,
            file_system_table: fst,
        };
        (gc_disc, reader)
    }

    fn fixed_size() -> Option<usize>
    {
        Some(0)
    }
}

impl<'a> GcDisc<'a>
{
    pub fn write<W: Write + Seek>(&mut self, writer: &mut W)
    {
        self.file_system_table.recalculate_offsets_and_lengths();
        self.file_system_table.write_files(writer);

        let main_dol_offset = self.file_system_table.fst_entries.iter()
            .find(|e| e.name.to_bytes() == "default.dol".as_bytes())
            .map(|e| e.offset)
            .expect("Couldn't find default.dol");

        // XXX It simplifies life a bit to just assume that the fst is at the same
        //     is the same length as before...

        self.header.main_dol_offset = main_dol_offset;

        writer.seek(SeekFrom::Start(0)).unwrap();

        self.header.write(writer);
        self.header_info.write(writer);
        self.apploader.write(writer);

        writer.seek(SeekFrom::Start(self.header.fst_offset as u64)).unwrap();
        self.file_system_table.write(writer);
    }
}

auto_struct! {
    #[auto_struct(Readable, FixedSize, Writable)]
    pub struct GcDiscHeader
    {
        console_id: u8,
        game_code: GenericArray<u8, U2>,
        country_code: u8,

        maker_code: GenericArray<u8, U2>,
        disc_id: u8,
        version: u8,

        audio_streaming: u8,
        stream_buffer_size: u8,

        unused0: GenericArray<u8, U18>, //[0x12]

        #[expect = 0xc2339f3d]
        magic: u32, // 0xc2339f3d
        game_name: GenericArray<u8, U992>, //[0x3e0]

        debug_mon_offset: u32,
        debug_mon_load_addr: u32,

        unused1: GenericArray<u8, U24>,// [0x18]

        main_dol_offset: u32,

        fst_offset: u32,
        fst_length: u32,
        fst_max_length: u32,

        user_position: u32,
        user_length: u32,

        unused2: u32,
        unused3: u32,
    }
}


auto_struct! {
    #[auto_struct(Readable, Writable)]
    pub struct GcDiscApploader<'a>
    {
        date: GenericArray<u8, U16>,
        entrypoint: u32,
        size: u32,
        trailer_size: u32,
        // TODO: Is this size right?
        code: RoArray<'a, u8> = ((size + trailer_size) as usize, ())
    }
}


pub struct FileSystemTable<'a>
{
    pub fst_entries: Vec<FstEntry<'a>>,
    string_table: Reader<'a>,
}

impl<'a> Readable<'a> for FileSystemTable<'a>
{
    type Args = (Reader<'a>, usize, usize);
    fn read(reader: Reader<'a>, args: (Reader<'a>, usize, usize))
        -> (FileSystemTable<'a>, Reader<'a>)
    {
        let (disc_start, fst_offset, total_size) = args;
        let fst_start = disc_start.offset(fst_offset as usize);

        // We lie initially to about the start of the string table because we
        // actually need the first fst entry to find the start of the string table.
        let root_fst_entry: FstEntry = fst_start.clone()
                                    .read((disc_start.clone(), disc_start.clone()));

        let fst_len = root_fst_entry.length as usize;
        let string_table_start = fst_start.offset(fst_len * FstEntry::fixed_size().unwrap());

        let fst_entries: Vec<FstEntry> = fst_start.clone()
            .read((fst_len, (disc_start, string_table_start.clone())));
        let string_table = string_table_start.truncated(total_size - fst_len);

        (FileSystemTable {
            fst_entries: fst_entries,
            string_table: string_table,
        }, reader)
    }

    fn size(&self) -> usize
    {
        self.fst_entries.size() + self.string_table.size()
    }
}

impl<'a> Writable for FileSystemTable<'a>
{
    fn write<W: Write>(&self, writer: &mut W)
    {
        self.fst_entries.write(writer);
        writer.write_all(&self.string_table).unwrap();
    }
}

impl<'a> FileSystemTable<'a>
{
    /// Updates the length and offset fields
    fn recalculate_offsets_and_lengths(&mut self)
    {
        // Get a list of all of the files in reverse order of their offsets' from
        // the start of the disc.
        let mut entries : Vec<_> = self.fst_entries.iter_mut()
            .filter(|e| !e.is_folder())
            .collect();
        entries.sort_by(|l, r| l.offset.cmp(&r.offset).reverse());

        let mut last_file_offset = GC_DISC_LENGTH as u32;
        for e in entries {
            e.length = e.file().unwrap().size() as u32;
            // We need to round up to a mupliple of 32
            last_file_offset -= (e.length + 31) & (u32::max_value() - 31);
            e.offset = last_file_offset;
        }
    }

    fn write_files<W: Write + Seek>(&self, writer: &mut W)
    {
        // TODO: If the files were sorted by offset, would that improve
        //       peformance?
        for e in self.fst_entries.iter() {
            let f = e.file();
            if f.is_none() {
                continue
            }
            let f = f.unwrap();
            writer.seek(SeekFrom::Start(e.offset as u64)).unwrap();
            f.write(writer)
        }
    }
}


auto_struct! {
    #[auto_struct(Readable, FixedSize, Writable)]
    #[derive(Debug)]
    pub struct FstEntry<'a>
    {
        #[args]
        (disc_start, string_table): (Reader<'a>, Reader<'a>),

        flags: u8,
        unused: u8,
        name_offset: u16,

        offset: u32,
        length: u32,

        #[literal]
        file: FstEntryFile<'a> = FstEntryFile::Unknown(disc_start.offset(offset as usize)
                                                            .truncated(length as usize)),
        #[literal]
        name: CStr<'a> = string_table.offset(name_offset as usize).read::<CStr<'a>>(()),
    }
}

// A wrapper around Box<Read> to make it impl Debug
pub struct ReadWrapper<'a>(RefCell<Box<Read + 'a>>);

#[derive(Debug)]
pub enum FstEntryFile<'a>
{
    Pak(LazySized<'a, Pak<'a>>),
    ExternalFile(ReadWrapper<'a>, usize),
    Unknown(Reader<'a>),
}

impl<'a> FstEntry<'a>
{
    pub fn file(&self) -> Option<&FstEntryFile<'a>>
    {
        if self.is_folder() {
            None
        } else {
            Some(&self.file)
        }
    }

    pub fn file_mut(&mut self) -> Option<&mut FstEntryFile<'a>>
    {
        if self.is_folder() {
            None
        } else {
            Some(&mut self.file)
        }
    }

    pub fn is_folder(&self) -> bool
    {
        self.flags == 1
    }

    pub fn guess_kind(&mut self)
    {
        let name = self.name.to_bytes();
        let len = name.len();

        // For simplicity's sake, assume all extentions are len 3
        let mut ext = [name[len - 3], name[len - 2], name[len - 1]];
        ext.make_ascii_lowercase();

        if ext == *b"pak" { 
            self.file = match self.file {
                FstEntryFile::Unknown(ref reader)
                    => FstEntryFile::Pak(reader.clone().read((reader.len(), ()))),
                FstEntryFile::Pak(_) => return,
                _ => panic!("Unexpected fst file type while trying to guess pak."),
            }
        }
    }
}

impl<'a> FstEntryFile<'a>
{
    fn size(&self) -> usize
    {
        match *self {
            FstEntryFile::Pak(ref pak) => pak.size(),
            FstEntryFile::ExternalFile(_, size) => size,
            FstEntryFile::Unknown(ref reader) => reader.len(),
        }
    }

    fn write<W: Write>(&self, writer: &mut W)
    {
        match *self {
            FstEntryFile::Pak(ref pak) => pak.write(writer),
            FstEntryFile::ExternalFile(ref file, _) => {
                let mut buf = [0u8; 4096];
                let mut file = file.0.borrow_mut();
                loop {
                    let read = file.read(&mut buf).unwrap();
                    if read == 0 {
                        break
                    };
                    writer.write_all(&buf[0..read]).unwrap();
                };
            },
            FstEntryFile::Unknown(ref reader) => writer.write_all(&reader).unwrap(),
        }
    }
}

impl<'a> ReadWrapper<'a>
{
    pub fn new<R: Read + 'static>(r: R) -> ReadWrapper<'a>
    {
        ReadWrapper(RefCell::new(Box::new(r) as Box<Read>))
    }
}
impl<'a> fmt::Debug for ReadWrapper<'a>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "Box<Read>")
    }
}
