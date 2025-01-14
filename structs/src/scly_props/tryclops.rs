use auto_struct_macros::auto_struct;
use reader_writer::CStr;
use reader_writer::typenum::*;
use reader_writer::generic_array::GenericArray;
use crate::scly_props::structs::*;
use crate::SclyPropertyData;
use crate::{impl_position, impl_rotation, impl_scale, impl_patterned_info_with_auxillary};

#[auto_struct(Readable, Writable)]
#[derive(Debug, Clone)]
pub struct Tryclops<'r>
{
    #[auto_struct(expect = 10)]
    pub prop_count: u32,

    pub name: CStr<'r>,

    pub position: GenericArray<f32, U3>,
    pub rotation: GenericArray<f32, U3>,
    pub scale: GenericArray<f32, U3>,

    pub patterned_info: PatternedInfo,
    pub actor_parameters: ActorParameters,

    pub dont_cares1: GenericArray<f32, U4>,
}

impl<'r> SclyPropertyData for Tryclops<'r>
{
    const OBJECT_TYPE: u8 = 0x7A;

    impl_position!();
    impl_rotation!();
    impl_scale!();
    impl_patterned_info_with_auxillary!();
}
