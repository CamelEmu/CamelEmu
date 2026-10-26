use emu_spirv::{Id, glsl};

use super::FragmentModule;
use crate::pica;

impl FragmentModule {
    pub(crate) fn write_tev_stage(&mut self, index: i32) {
        let stage = self.config.texture.tev_stages[index as usize];

        let is_passthrough_tev_stage = pica::tev_operation(stage.color_op()).is_replace()
            && pica::tev_operation(stage.alpha_op()).is_replace()
            && pica::tev_source(stage.color_source1()).is_previous()
            && pica::tev_source(stage.alpha_source1()).is_previous()
            && pica::color_modifier(stage.color_modifier1()).is_source_color()
            && pica::alpha_modifier(stage.alpha_modifier1()).is_source_alpha()
            && stage.color_multiplier() == 1
            && stage.alpha_multiplier() == 1;

        if !is_passthrough_tev_stage {
            let (f32_id, vec3, vec4) = (self.f32_id, self.vec_ids.get(3), self.vec_ids.get(4));

            self.color_results_1 =
                self.append_color_modifier(stage.color_modifier1(), stage.color_source1(), index);
            self.color_results_2 =
                self.append_color_modifier(stage.color_modifier2(), stage.color_source2(), index);
            self.color_results_3 =
                self.append_color_modifier(stage.color_modifier3(), stage.color_source3(), index);

            let color_operation = pica::tev_operation(stage.color_op());
            let combined = self.append_color_combiner(stage.color_op());
            let mut color_output = self.byteround(combined, 3);
            let mut alpha_output;

            if color_operation.is_dot3_rgba() {
                alpha_output = self.composite_extract(f32_id, color_output, 0);
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

                let combined = self.append_alpha_combiner(stage.alpha_op());
                alpha_output = self.byteround(combined, 1);
            }

            let color_multiplier = self.const_f32(stage.color_multiplier() as f32);
            color_output = self.op_vector_times_scalar(vec3, color_output, color_multiplier);
            let zeros = self.const_f32s(&[0.0, 0.0, 0.0]);
            let ones = self.const_f32s(&[1.0, 1.0, 1.0]);
            color_output = self.glsl::<glsl::FClamp>(vec3, &[color_output, zeros, ones]);
            let alpha_multiplier = self.const_f32(stage.alpha_multiplier() as f32);
            alpha_output = self.op_f_mul(f32_id, alpha_output, alpha_multiplier);
            let zero = self.const_f32(0.0);
            let one = self.const_f32(1.0);
            alpha_output = self.glsl::<glsl::FClamp>(f32_id, &[alpha_output, zero, one]);
            self.combiner_output =
                self.op_composite_construct(vec4, &[color_output, alpha_output]);
        }

        let vec4 = self.vec_ids.get(4);
        self.combiner_buffer = self.next_combiner_buffer;
        if self
            .config
            .tev_stage_updates_combiner_buffer_color(index as u32)
        {
            let (combiner_output, next_combiner_buffer) =
                (self.combiner_output, self.next_combiner_buffer);
            self.next_combiner_buffer = self.vector_shuffle(
                vec4,
                combiner_output,
                next_combiner_buffer,
                &[0, 1, 2, 7],
            );
        }

        if self
            .config
            .tev_stage_updates_combiner_buffer_alpha(index as u32)
        {
            let (combiner_output, next_combiner_buffer) =
                (self.combiner_output, self.next_combiner_buffer);
            self.next_combiner_buffer = self.vector_shuffle(
                vec4,
                next_combiner_buffer,
                combiner_output,
                &[0, 1, 2, 7],
            );
        }
    }

    fn append_color_modifier(&mut self, modifier_raw: u32, source_raw: u32, index: i32) -> Id {
        let stage = self.config.texture.tev_stages[index as usize];
        let force_source3 = index == 0 && pica::tev_source(source_raw).is_previous();
        let source = if force_source3 {
            stage.color_source3()
        } else {
            source_raw
        };
        let source_color = pica::tev_source(source).get(self, index);
        let one_vec = self.const_f32s(&[1.0, 1.0, 1.0]);
        pica::color_modifier(modifier_raw).apply(self, source_color, one_vec)
    }

    fn append_alpha_modifier(&mut self, modifier_raw: u32, source_raw: u32, index: i32) -> Id {
        let stage = self.config.texture.tev_stages[index as usize];
        let force_source3 = index == 0 && pica::tev_source(source_raw).is_previous();
        let source = if force_source3 {
            stage.alpha_source3()
        } else {
            source_raw
        };
        let source_alpha = pica::tev_source(source).get(self, index);
        let one_f32 = self.const_f32(1.0);
        pica::alpha_modifier(modifier_raw).apply(self, source_alpha, one_f32)
    }

    fn append_color_combiner(&mut self, operation_raw: u32) -> Id {
        let vec3 = self.vec_ids.get(3);
        let half_vec = self.const_f32s(&[0.5, 0.5, 0.5]);
        let one_vec = self.const_f32s(&[1.0, 1.0, 1.0]);
        let zero_vec = self.const_f32s(&[0.0, 0.0, 0.0]);

        let color = pica::tev_operation(operation_raw).color(self, half_vec, one_vec, zero_vec);

        self.glsl::<glsl::FClamp>(vec3, &[color, zero_vec, one_vec])
    }

    fn append_alpha_combiner(&mut self, operation_raw: u32) -> Id {
        let f32_id = self.f32_id;
        let color = pica::tev_operation(operation_raw).alpha(self);

        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        self.glsl::<glsl::FClamp>(f32_id, &[color, zero, one])
    }
}
