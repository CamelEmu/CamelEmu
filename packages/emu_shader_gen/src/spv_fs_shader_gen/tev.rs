use emu_pica_types::regs::framebuffer::CompareFunc;
use emu_pica_types::regs::texturing::{
    TevAlphaModifier, TevColorModifier, TevOperation, TevSource, TevStageConfig,
};
use rspirv::spirv::SelectionControl;

use super::FragmentModule;
use crate::spv_module::Id;

/// Detects if a TEV stage is configured to be skipped (to avoid generating unnecessary code)
fn is_passthrough_tev_stage(stage: &TevStageConfig) -> bool {
    stage.color_op() == TevOperation::REPLACE
        && stage.alpha_op() == TevOperation::REPLACE
        && stage.color_source1() == TevSource::PREVIOUS
        && stage.alpha_source1() == TevSource::PREVIOUS
        && stage.color_modifier1() == TevColorModifier::SOURCE_COLOR
        && stage.alpha_modifier1() == TevAlphaModifier::SOURCE_ALPHA
        && stage.get_color_multiplier() == 1
        && stage.get_alpha_multiplier() == 1
}

impl FragmentModule<'_> {
    /// Writes the code to emulate the specified TEV stage
    pub(super) fn write_tev_stage(&mut self, index: i32) {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[index as usize]);
        let f32_id = self.f32_id;
        let vec3 = self.vec_ids.get(3);
        let vec4 = self.vec_ids.get(4);

        if !is_passthrough_tev_stage(&stage) {
            self.color_results_1 =
                self.append_color_modifier(stage.color_modifier1(), stage.color_source1(), index);
            self.color_results_2 =
                self.append_color_modifier(stage.color_modifier2(), stage.color_source2(), index);
            self.color_results_3 =
                self.append_color_modifier(stage.color_modifier3(), stage.color_source3(), index);

            // Round the output of each TEV stage to maintain the PICA's 8 bits of precision
            let color_combined = self.append_color_combiner(stage.color_op());
            let mut color_output = self.byteround(color_combined, 3);
            let mut alpha_output;

            if stage.color_op() == TevOperation::DOT3_RGBA {
                // result of Dot3_RGBA operation is also placed to the alpha component
                alpha_output = self.m.op_composite_extract(f32_id, color_output, 0);
            } else {
                self.alpha_results_1 = self.append_alpha_modifier(
                    stage.alpha_modifier1(),
                    stage.alpha_source1(),
                    index,
                );
                self.alpha_results_2 = self.append_alpha_modifier(
                    stage.alpha_modifier2(),
                    stage.alpha_source2(),
                    index,
                );
                self.alpha_results_3 = self.append_alpha_modifier(
                    stage.alpha_modifier3(),
                    stage.alpha_source3(),
                    index,
                );

                let alpha_combined = self.append_alpha_combiner(stage.alpha_op());
                alpha_output = self.byteround(alpha_combined, 1);
            }

            let color_multiplier = self.const_f32(stage.get_color_multiplier() as f32);
            color_output = self
                .m
                .op_vector_times_scalar(vec3, color_output, color_multiplier);
            let zero_vec = self.const_f32_vec(&[0.0, 0.0, 0.0]);
            let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0]);
            color_output = self.m.op_f_clamp(vec3, color_output, zero_vec, one_vec);
            let alpha_multiplier = self.const_f32(stage.get_alpha_multiplier() as f32);
            alpha_output = self.m.op_f_mul(f32_id, alpha_output, alpha_multiplier);
            let zero = self.const_f32(0.0);
            let one = self.const_f32(1.0);
            alpha_output = self.m.op_f_clamp(f32_id, alpha_output, zero, one);
            self.combiner_output = self
                .m
                .op_composite_construct(vec4, &[color_output, alpha_output]);
        }

        self.combiner_buffer = self.next_combiner_buffer;
        if self
            .config
            .tev_stage_updates_combiner_buffer_color(index as u32)
        {
            self.next_combiner_buffer = self.m.op_vector_shuffle(
                vec4,
                self.combiner_output,
                self.next_combiner_buffer,
                &[0, 1, 2, 7],
            );
        }

        if self
            .config
            .tev_stage_updates_combiner_buffer_alpha(index as u32)
        {
            self.next_combiner_buffer = self.m.op_vector_shuffle(
                vec4,
                self.next_combiner_buffer,
                self.combiner_output,
                &[0, 1, 2, 7],
            );
        }
    }

    /// Writes the if-statement condition used to evaluate alpha testing.
    pub(super) fn write_alpha_test_condition(&mut self, func: CompareFunc) {
        // Don't check for kill, this is done earlier
        match func {
            CompareFunc::ALWAYS => {} // Do nothing
            CompareFunc::EQUAL
            | CompareFunc::NOT_EQUAL
            | CompareFunc::LESS_THAN
            | CompareFunc::LESS_THAN_OR_EQUAL
            | CompareFunc::GREATER_THAN
            | CompareFunc::GREATER_THAN_OR_EQUAL => {
                let f32_id = self.f32_id;
                let alpha = self.m.op_composite_extract(f32_id, self.combiner_output, 3);
                let scale = self.const_f32(255.0);
                let alpha_scaled = self.m.op_f_mul(f32_id, alpha, scale);
                let alpha_int = self.m.op_convert_f_to_s(self.i32_id, alpha_scaled);
                let member = self.const_s32(1);
                let alphatest_ref = self.get_shader_data_member(self.i32_id, &[member]);
                let alpha_comp_ref = self.compare(func, alpha_int, alphatest_ref);
                let kill_label = self.m.op_label();
                let keep_label = self.m.op_label();
                self.m
                    .op_selection_merge(keep_label, SelectionControl::NONE);
                self.m
                    .op_branch_conditional(alpha_comp_ref, kill_label, keep_label);
                self.m.add_label(kill_label);
                self.m.op_kill();
                self.m.add_label(keep_label);
            }
            _ => {
                log::error!(target: "Render", "Unknown alpha test condition {}", func.0);
            }
        }
    }

    /// The compare func is to keep the fragment so we invert it to discard it
    fn compare(&mut self, func: CompareFunc, alpha: Id, alphatest_ref: Id) -> Id {
        let bool_id = self.bool_id;
        match func {
            CompareFunc::EQUAL => self.m.op_i_not_equal(bool_id, alpha, alphatest_ref),
            CompareFunc::NOT_EQUAL => self.m.op_i_equal(bool_id, alpha, alphatest_ref),
            CompareFunc::LESS_THAN => self
                .m
                .op_s_greater_than_equal(bool_id, alpha, alphatest_ref),
            CompareFunc::LESS_THAN_OR_EQUAL => {
                self.m.op_s_greater_than(bool_id, alpha, alphatest_ref)
            }
            CompareFunc::GREATER_THAN => self.m.op_s_less_than_equal(bool_id, alpha, alphatest_ref),
            CompareFunc::GREATER_THAN_OR_EQUAL => {
                self.m.op_s_less_than(bool_id, alpha, alphatest_ref)
            }
            _ => 0,
        }
    }

    /// Returns the specified TEV stage source component(s)
    fn get_source(&mut self, source: TevSource, index: i32) -> Id {
        let vec4 = self.vec_ids.get(4);
        match source {
            TevSource::PRIMARY_COLOR => self.rounded_primary_color,
            TevSource::PRIMARY_FRAGMENT_COLOR => self.primary_fragment_color,
            TevSource::SECONDARY_FRAGMENT_COLOR => self.secondary_fragment_color,
            TevSource::TEXTURE0 => self.m.op_function_call(vec4, self.sample_tex_unit_func[0]),
            TevSource::TEXTURE1 => self.m.op_function_call(vec4, self.sample_tex_unit_func[1]),
            TevSource::TEXTURE2 => self.m.op_function_call(vec4, self.sample_tex_unit_func[2]),
            TevSource::TEXTURE3 => self.m.op_function_call(vec4, self.sample_tex_unit_func[3]),
            TevSource::PREVIOUS_BUFFER => self.combiner_buffer,
            TevSource::CONSTANT => {
                let member = self.const_s32(25);
                let index = self.const_s32(index);
                self.get_shader_data_member(vec4, &[member, index])
            }
            TevSource::PREVIOUS => self.combiner_output,
            _ => {
                log::error!(target: "Render", "Unknown source op {}", source.0);
                self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0])
            }
        }
    }

    /// Writes the color components to use for the specified TEV stage color modifier
    fn append_color_modifier(
        &mut self,
        modifier: TevColorModifier,
        source: TevSource,
        index: i32,
    ) -> Id {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[index as usize]);
        let force_source3 = index == 0 && source == TevSource::PREVIOUS;
        let source_color = self.get_source(
            if force_source3 {
                stage.color_source3()
            } else {
                source
            },
            index,
        );
        let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0]);
        let vec3 = self.vec_ids.get(3);

        let shuffle = |module: &mut Self, r: u32, g: u32, b: u32| -> Id {
            module
                .m
                .op_vector_shuffle(vec3, source_color, source_color, &[r, g, b])
        };

        match modifier {
            TevColorModifier::SOURCE_COLOR => shuffle(self, 0, 1, 2),
            TevColorModifier::ONE_MINUS_SOURCE_COLOR => {
                let color = shuffle(self, 0, 1, 2);
                self.m.op_f_sub(vec3, one_vec, color)
            }
            TevColorModifier::SOURCE_RED => shuffle(self, 0, 0, 0),
            TevColorModifier::ONE_MINUS_SOURCE_RED => {
                let color = shuffle(self, 0, 0, 0);
                self.m.op_f_sub(vec3, one_vec, color)
            }
            TevColorModifier::SOURCE_GREEN => shuffle(self, 1, 1, 1),
            TevColorModifier::ONE_MINUS_SOURCE_GREEN => {
                let color = shuffle(self, 1, 1, 1);
                self.m.op_f_sub(vec3, one_vec, color)
            }
            TevColorModifier::SOURCE_BLUE => shuffle(self, 2, 2, 2),
            TevColorModifier::ONE_MINUS_SOURCE_BLUE => {
                let color = shuffle(self, 2, 2, 2);
                self.m.op_f_sub(vec3, one_vec, color)
            }
            TevColorModifier::SOURCE_ALPHA => shuffle(self, 3, 3, 3),
            TevColorModifier::ONE_MINUS_SOURCE_ALPHA => {
                let color = shuffle(self, 3, 3, 3);
                self.m.op_f_sub(vec3, one_vec, color)
            }
            _ => {
                log::error!(target: "Render", "Unknown color modifier op {}", modifier.0);
                one_vec
            }
        }
    }

    /// Writes the alpha component to use for the specified TEV stage alpha modifier
    fn append_alpha_modifier(
        &mut self,
        modifier: TevAlphaModifier,
        source: TevSource,
        index: i32,
    ) -> Id {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[index as usize]);
        let force_source3 = index == 0 && source == TevSource::PREVIOUS;
        let source_alpha = self.get_source(
            if force_source3 {
                stage.alpha_source3()
            } else {
                source
            },
            index,
        );
        let one_f32 = self.const_f32(1.0);
        let f32_id = self.f32_id;

        let component = |module: &mut Self, c: u32| -> Id {
            module.m.op_composite_extract(f32_id, source_alpha, c)
        };

        match modifier {
            TevAlphaModifier::SOURCE_ALPHA => component(self, 3),
            TevAlphaModifier::ONE_MINUS_SOURCE_ALPHA => {
                let alpha = component(self, 3);
                self.m.op_f_sub(f32_id, one_f32, alpha)
            }
            TevAlphaModifier::SOURCE_RED => component(self, 0),
            TevAlphaModifier::ONE_MINUS_SOURCE_RED => {
                let alpha = component(self, 0);
                self.m.op_f_sub(f32_id, one_f32, alpha)
            }
            TevAlphaModifier::SOURCE_GREEN => component(self, 1),
            TevAlphaModifier::ONE_MINUS_SOURCE_GREEN => {
                let alpha = component(self, 1);
                self.m.op_f_sub(f32_id, one_f32, alpha)
            }
            TevAlphaModifier::SOURCE_BLUE => component(self, 2),
            TevAlphaModifier::ONE_MINUS_SOURCE_BLUE => {
                let alpha = component(self, 2);
                self.m.op_f_sub(f32_id, one_f32, alpha)
            }
            _ => {
                log::error!(target: "Render", "Unknown alpha modifier op {}", modifier.0);
                one_f32
            }
        }
    }

    /// Writes the combiner function for the color components for the specified TEV stage operation
    fn append_color_combiner(&mut self, operation: TevOperation) -> Id {
        let vec3 = self.vec_ids.get(3);
        let f32_id = self.f32_id;
        let half_vec = self.const_f32_vec(&[0.5, 0.5, 0.5]);
        let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0]);
        let zero_vec = self.const_f32_vec(&[0.0, 0.0, 0.0]);
        let (results_1, results_2, results_3) = (
            self.color_results_1,
            self.color_results_2,
            self.color_results_3,
        );

        let color = match operation {
            TevOperation::REPLACE => results_1,
            TevOperation::MODULATE => self.m.op_f_mul(vec3, results_1, results_2),
            TevOperation::ADD => self.m.op_f_add(vec3, results_1, results_2),
            TevOperation::ADD_SIGNED => {
                let sum = self.m.op_f_add(vec3, results_1, results_2);
                self.m.op_f_sub(vec3, sum, half_vec)
            }
            TevOperation::LERP => self.m.op_f_mix(vec3, results_2, results_1, results_3),
            TevOperation::SUBTRACT => self.m.op_f_sub(vec3, results_1, results_2),
            TevOperation::MULTIPLY_THEN_ADD => self.m.op_fma(vec3, results_1, results_2, results_3),
            TevOperation::ADD_THEN_MULTIPLY => {
                let sum = self.m.op_f_add(vec3, results_1, results_2);
                let color = self.m.op_f_min(vec3, sum, one_vec);
                self.m.op_f_mul(vec3, color, results_3)
            }
            TevOperation::DOT3_RGB | TevOperation::DOT3_RGBA => {
                let lhs = self.m.op_f_sub(vec3, results_1, half_vec);
                let rhs = self.m.op_f_sub(vec3, results_2, half_vec);
                let dot = self.m.op_dot(f32_id, lhs, rhs);
                let four = self.const_f32(4.0);
                let color = self.m.op_f_mul(f32_id, dot, four);
                self.m.op_composite_construct(vec3, &[color, color, color])
            }
            _ => {
                log::error!(target: "Render", "Unknown color combiner operation: {}", operation.0);
                zero_vec
            }
        };

        // Clamp result to 0.0, 1.0
        self.m.op_f_clamp(vec3, color, zero_vec, one_vec)
    }

    /// Writes the combiner function for the alpha component for the specified TEV stage operation
    fn append_alpha_combiner(&mut self, operation: TevOperation) -> Id {
        let f32_id = self.f32_id;
        let (results_1, results_2, results_3) = (
            self.alpha_results_1,
            self.alpha_results_2,
            self.alpha_results_3,
        );

        let color = match operation {
            TevOperation::REPLACE => results_1,
            TevOperation::MODULATE => self.m.op_f_mul(f32_id, results_1, results_2),
            TevOperation::ADD => self.m.op_f_add(f32_id, results_1, results_2),
            TevOperation::ADD_SIGNED => {
                let sum = self.m.op_f_add(f32_id, results_1, results_2);
                let half = self.const_f32(0.5);
                self.m.op_f_sub(f32_id, sum, half)
            }
            TevOperation::LERP => self.m.op_f_mix(f32_id, results_2, results_1, results_3),
            TevOperation::SUBTRACT => self.m.op_f_sub(f32_id, results_1, results_2),
            TevOperation::MULTIPLY_THEN_ADD => {
                self.m.op_fma(f32_id, results_1, results_2, results_3)
            }
            TevOperation::ADD_THEN_MULTIPLY => {
                let sum = self.m.op_f_add(f32_id, results_1, results_2);
                let one = self.const_f32(1.0);
                let color = self.m.op_f_min(f32_id, sum, one);
                self.m.op_f_mul(f32_id, color, results_3)
            }
            _ => {
                log::error!(target: "Render", "Unknown alpha combiner operation: {}", operation.0);
                self.const_f32(0.0)
            }
        };

        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        self.m.op_f_clamp(f32_id, color, zero, one)
    }
}
