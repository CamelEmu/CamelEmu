use crate::id::Id;
use crate::module::Module;

macro_rules! image_instructions {
    ($($name:ident => $op:ident;)*) => {
        impl Module {
            $(
                pub fn $name(
                    &mut self,
                    result_type: Id,
                    sampled_image: Id,
                    coordinate: Id,
                    image_operands: Option<spirv::ImageOperands>,
                    operands: &[Id],
                ) -> Id {
                    self.code.emit_typed(
                        &mut self.bound,
                        6 + operands.len(),
                        spirv::Op::$op,
                        result_type,
                        &[&sampled_image, &coordinate, &image_operands, &operands],
                    )
                }
            )*
        }
    };
}

macro_rules! image_explicit_instructions {
    ($($name:ident => $op:ident;)*) => {
        impl Module {
            $(
                pub fn $name(
                    &mut self,
                    result_type: Id,
                    sampled_image: Id,
                    coordinate: Id,
                    image_operands: spirv::ImageOperands,
                    operands: &[Id],
                ) -> Id {
                    self.code.emit_typed(
                        &mut self.bound,
                        6 + operands.len(),
                        spirv::Op::$op,
                        result_type,
                        &[&sampled_image, &coordinate, &image_operands, &operands],
                    )
                }
            )*
        }
    };
}

macro_rules! image_extra_instructions {
    ($($name:ident => $op:ident;)*) => {
        impl Module {
            $(
                pub fn $name(
                    &mut self,
                    result_type: Id,
                    sampled_image: Id,
                    coordinate: Id,
                    extra: Id,
                    image_operands: Option<spirv::ImageOperands>,
                    operands: &[Id],
                ) -> Id {
                    self.code.emit_typed(
                        &mut self.bound,
                        7 + operands.len(),
                        spirv::Op::$op,
                        result_type,
                        &[&sampled_image, &coordinate, &extra, &image_operands, &operands],
                    )
                }
            )*
        }
    };
}

macro_rules! image_extra_explicit_instructions {
    ($($name:ident => $op:ident;)*) => {
        impl Module {
            $(
                pub fn $name(
                    &mut self,
                    result_type: Id,
                    sampled_image: Id,
                    coordinate: Id,
                    extra: Id,
                    image_operands: spirv::ImageOperands,
                    operands: &[Id],
                ) -> Id {
                    self.code.emit_typed(
                        &mut self.bound,
                        8 + operands.len(),
                        spirv::Op::$op,
                        result_type,
                        &[&sampled_image, &coordinate, &extra, &image_operands, &operands],
                    )
                }
            )*
        }
    };
}

image_instructions! {
    op_image_sample_implicit_lod => ImageSampleImplicitLod;
    op_image_sample_proj_implicit_lod => ImageSampleProjImplicitLod;
    op_image_fetch => ImageFetch;
    op_image_read => ImageRead;
}

image_explicit_instructions! {
    op_image_sample_explicit_lod => ImageSampleExplicitLod;
    op_image_sample_proj_explicit_lod => ImageSampleProjExplicitLod;
}

image_extra_instructions! {
    op_image_sample_dref_implicit_lod => ImageSampleDrefImplicitLod;
    op_image_sample_proj_dref_implicit_lod => ImageSampleProjDrefImplicitLod;
    op_image_gather => ImageGather;
    op_image_dref_gather => ImageDrefGather;
}

image_extra_explicit_instructions! {
    op_image_sample_dref_explicit_lod => ImageSampleDrefExplicitLod;
    op_image_sample_proj_dref_explicit_lod => ImageSampleProjDrefExplicitLod;
}

typed_instructions! {
    op_image_query_size_lod(image, extra) => ImageQuerySizeLod;
    op_image_query_lod(image, extra) => ImageQueryLod;
    op_sampled_image(image, sampler) => SampledImage;
    op_image(sampled_image) => Image;
    op_image_sparse_texels_resident(resident_code) => ImageSparseTexelsResident;
}

macro_rules! image_query_instructions {
    ($($name:ident => $op:ident;)*) => {
        impl Module {
            $(
                pub fn $name(&mut self, result_type: Id, image: Id) -> Id {
                    self.code.emit_typed(
                        &mut self.bound,
                        5,
                        spirv::Op::$op,
                        result_type,
                        &[&image],
                    )
                }
            )*
        }
    };
}

image_query_instructions! {
    op_image_query_size => ImageQuerySize;
    op_image_query_levels => ImageQueryLevels;
    op_image_query_samples => ImageQuerySamples;
}

impl Module {
    pub fn op_image_write(
        &mut self,
        image: Id,
        coordinate: Id,
        texel: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        debug_assert!(image_operands.is_some() != operands.is_empty());
        self.code.emit(
            &mut self.bound,
            5 + operands.len(),
            spirv::Op::ImageWrite,
            &[&image, &coordinate, &texel, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_sample_implicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + usize::from(image_operands.is_some()) + operands.len(),
            spirv::Op::ImageSparseSampleImplicitLod,
            result_type,
            &[&sampled_image, &coordinate, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_sample_explicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        image_operands: spirv::ImageOperands,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6 + operands.len(),
            spirv::Op::ImageSparseSampleExplicitLod,
            result_type,
            &[&sampled_image, &coordinate, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_sample_dref_implicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        dref: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6 + usize::from(image_operands.is_some()) + operands.len(),
            spirv::Op::ImageSparseSampleDrefImplicitLod,
            result_type,
            &[&sampled_image, &coordinate, &dref, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_sample_dref_explicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        dref: Id,
        image_operands: spirv::ImageOperands,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            7 + operands.len(),
            spirv::Op::ImageSparseSampleDrefExplicitLod,
            result_type,
            &[&sampled_image, &coordinate, &dref, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_fetch(
        &mut self,
        result_type: Id,
        image: Id,
        coordinate: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + usize::from(image_operands.is_some()) + operands.len(),
            spirv::Op::ImageSparseFetch,
            result_type,
            &[&image, &coordinate, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_gather(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        component: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6 + operands.len(),
            spirv::Op::ImageSparseGather,
            result_type,
            &[&sampled_image, &coordinate, &component, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_dref_gather(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        dref: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6 + operands.len(),
            spirv::Op::ImageSparseDrefGather,
            result_type,
            &[&sampled_image, &coordinate, &dref, &image_operands, &operands],
        )
    }

    pub fn op_image_sparse_read(
        &mut self,
        result_type: Id,
        image: Id,
        coordinate: Id,
        image_operands: Option<spirv::ImageOperands>,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + usize::from(image_operands.is_some()) + operands.len(),
            spirv::Op::ImageSparseTexelsResident,
            result_type,
            &[&image, &coordinate, &image_operands, &operands],
        )
    }
}
