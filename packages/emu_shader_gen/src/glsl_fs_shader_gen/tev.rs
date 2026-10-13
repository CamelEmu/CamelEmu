use emu_pica_types::regs::framebuffer::CompareFunc;
use emu_pica_types::regs::texturing::{
    TevAlphaModifier, TevColorModifier, TevOperation, TevSource, TevStageConfig,
};

use super::FragmentModule;

fn is_pass_through_tev_stage(stage: &TevStageConfig) -> bool {
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
    /// Returns the specified TEV stage source component(s)
    fn get_source(&self, source: TevSource, tev_index: u32) -> String {
        match source {
            TevSource::PRIMARY_COLOR => "rounded_primary_color".to_string(),
            TevSource::PRIMARY_FRAGMENT_COLOR => "primary_fragment_color".to_string(),
            TevSource::SECONDARY_FRAGMENT_COLOR => "secondary_fragment_color".to_string(),
            TevSource::TEXTURE0 => "sampleTexUnit0()".to_string(),
            TevSource::TEXTURE1 => "sampleTexUnit1()".to_string(),
            TevSource::TEXTURE2 => "sampleTexUnit2()".to_string(),
            TevSource::TEXTURE3 => "sampleTexUnit3()".to_string(),
            TevSource::PREVIOUS_BUFFER => "combiner_buffer".to_string(),
            TevSource::CONSTANT => format!("const_color[{}]", tev_index),
            TevSource::PREVIOUS => "combiner_output".to_string(),
            _ => {
                log::error!(target: "Render", "Unknown source op {}", source.0);
                "vec4(0.0)".to_string()
            }
        }
    }

    /// Writes the color components to use for the specified TEV stage color modifier
    fn append_color_modifier(
        &mut self,
        modifier: TevColorModifier,
        source: TevSource,
        tev_index: u32,
    ) {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[tev_index as usize]);
        let force_source3 = tev_index == 0 && source == TevSource::PREVIOUS;
        let color_source = self.get_source(
            if force_source3 {
                stage.color_source3()
            } else {
                source
            },
            tev_index,
        );
        match modifier {
            TevColorModifier::SOURCE_COLOR => {
                self.out += &format!("{}.rgb", color_source);
            }
            TevColorModifier::ONE_MINUS_SOURCE_COLOR => {
                self.out += &format!("vec3(1.0) - {}.rgb", color_source);
            }
            TevColorModifier::SOURCE_ALPHA => {
                self.out += &format!("{}.aaa", color_source);
            }
            TevColorModifier::ONE_MINUS_SOURCE_ALPHA => {
                self.out += &format!("vec3(1.0) - {}.aaa", color_source);
            }
            TevColorModifier::SOURCE_RED => {
                self.out += &format!("{}.rrr", color_source);
            }
            TevColorModifier::ONE_MINUS_SOURCE_RED => {
                self.out += &format!("vec3(1.0) - {}.rrr", color_source);
            }
            TevColorModifier::SOURCE_GREEN => {
                self.out += &format!("{}.ggg", color_source);
            }
            TevColorModifier::ONE_MINUS_SOURCE_GREEN => {
                self.out += &format!("vec3(1.0) - {}.ggg", color_source);
            }
            TevColorModifier::SOURCE_BLUE => {
                self.out += &format!("{}.bbb", color_source);
            }
            TevColorModifier::ONE_MINUS_SOURCE_BLUE => {
                self.out += &format!("vec3(1.0) - {}.bbb", color_source);
            }
            _ => {
                self.out += "vec3(0.0)";
                log::error!(target: "Render", "Unknown color modifier op {}", modifier.0);
            }
        }
    }

    /// Writes the alpha component to use for the specified TEV stage alpha modifier
    fn append_alpha_modifier(
        &mut self,
        modifier: TevAlphaModifier,
        source: TevSource,
        tev_index: u32,
    ) {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[tev_index as usize]);
        let force_source3 = tev_index == 0 && source == TevSource::PREVIOUS;
        let alpha_source = self.get_source(
            if force_source3 {
                stage.alpha_source3()
            } else {
                source
            },
            tev_index,
        );
        match modifier {
            TevAlphaModifier::SOURCE_ALPHA => {
                self.out += &format!("{}.a", alpha_source);
            }
            TevAlphaModifier::ONE_MINUS_SOURCE_ALPHA => {
                self.out += &format!("1.0 - {}.a", alpha_source);
            }
            TevAlphaModifier::SOURCE_RED => {
                self.out += &format!("{}.r", alpha_source);
            }
            TevAlphaModifier::ONE_MINUS_SOURCE_RED => {
                self.out += &format!("1.0 - {}.r", alpha_source);
            }
            TevAlphaModifier::SOURCE_GREEN => {
                self.out += &format!("{}.g", alpha_source);
            }
            TevAlphaModifier::ONE_MINUS_SOURCE_GREEN => {
                self.out += &format!("1.0 - {}.g", alpha_source);
            }
            TevAlphaModifier::SOURCE_BLUE => {
                self.out += &format!("{}.b", alpha_source);
            }
            TevAlphaModifier::ONE_MINUS_SOURCE_BLUE => {
                self.out += &format!("1.0 - {}.b", alpha_source);
            }
            _ => {
                self.out += "0.0";
                log::error!(target: "Render", "Unknown alpha modifier op {}", modifier.0);
            }
        }
    }

    /// Writes the combiner function for the color components for the specified TEV stage operation
    fn append_color_combiner(&mut self, operation: TevOperation) {
        let get_combiner = || match operation {
            TevOperation::REPLACE => "color_results_1",
            TevOperation::MODULATE => "color_results_1 * color_results_2",
            TevOperation::ADD => "color_results_1 + color_results_2",
            TevOperation::ADD_SIGNED => "color_results_1 + color_results_2 - vec3(0.5)",
            TevOperation::LERP => "mix(color_results_2, color_results_1, color_results_3)",
            TevOperation::SUBTRACT => "color_results_1 - color_results_2",
            TevOperation::MULTIPLY_THEN_ADD => {
                "fma(color_results_1, color_results_2, color_results_3)"
            }
            TevOperation::ADD_THEN_MULTIPLY => {
                "min(color_results_1 + color_results_2, vec3(1.0)) * color_results_3"
            }
            TevOperation::DOT3_RGB | TevOperation::DOT3_RGBA => {
                "vec3(dot(color_results_1 - vec3(0.5), color_results_2 - vec3(0.5)) * 4.0)"
            }
            _ => {
                log::error!(target: "Render", "Unknown color combiner operation: {}", operation.0);
                "vec3(0.0)"
            }
        };
        self.out += &format!("clamp({}, vec3(0.0), vec3(1.0))", get_combiner());
    }

    /// Writes the combiner function for the alpha component for the specified TEV stage operation
    fn append_alpha_combiner(&mut self, operation: TevOperation) {
        let get_combiner = || match operation {
            TevOperation::REPLACE => "alpha_results_1",
            TevOperation::MODULATE => "alpha_results_1 * alpha_results_2",
            TevOperation::ADD => "alpha_results_1 + alpha_results_2",
            TevOperation::ADD_SIGNED => "alpha_results_1 + alpha_results_2 - 0.5",
            TevOperation::LERP => "mix(alpha_results_2, alpha_results_1, alpha_results_3)",
            TevOperation::SUBTRACT => "alpha_results_1 - alpha_results_2",
            TevOperation::MULTIPLY_THEN_ADD => {
                "fma(alpha_results_1, alpha_results_2, alpha_results_3)"
            }
            TevOperation::ADD_THEN_MULTIPLY => {
                "min(alpha_results_1 + alpha_results_2, 1.0) * alpha_results_3"
            }
            _ => {
                log::error!(target: "Render", "Unknown alpha combiner operation: {}", operation.0);
                "0.0"
            }
        };
        self.out += &format!("clamp({}, 0.0, 1.0)", get_combiner());
    }

    /// Writes the if-statement condition used to evaluate alpha testing
    pub(super) fn write_alpha_test_condition(&mut self, func: CompareFunc) {
        let get_cond = || -> String {
            match func {
                CompareFunc::NEVER => "true".to_string(),
                CompareFunc::ALWAYS => "false".to_string(),
                CompareFunc::EQUAL
                | CompareFunc::NOT_EQUAL
                | CompareFunc::LESS_THAN
                | CompareFunc::LESS_THAN_OR_EQUAL
                | CompareFunc::GREATER_THAN
                | CompareFunc::GREATER_THAN_OR_EQUAL => {
                    const OP: [&str; 6] = ["!=", "==", ">=", ">", "<=", "<"];
                    let index = func.0 - CompareFunc::EQUAL.0;
                    format!(
                        "int(combiner_output.a * 255.0) {} alphatest_ref",
                        OP[index as usize]
                    )
                }
                _ => {
                    log::error!(target: "Render", "Unknown alpha test condition {}", func.0);
                    "false".to_string()
                }
            }
        };
        self.out += &format!("if ({}) discard;\n", get_cond());
    }

    /// Writes the code to emulate the specified TEV stage
    pub(super) fn write_tev_stage(&mut self, index: u32) {
        let stage = TevStageConfig::from(self.config.texture.tev_stages[index as usize]);
        if !is_pass_through_tev_stage(&stage) {
            self.out += "color_results_1 = ";
            self.append_color_modifier(stage.color_modifier1(), stage.color_source1(), index);
            self.out += ";\ncolor_results_2 = ";
            self.append_color_modifier(stage.color_modifier2(), stage.color_source2(), index);
            self.out += ";\ncolor_results_3 = ";
            self.append_color_modifier(stage.color_modifier3(), stage.color_source3(), index);

            // Round the output of each TEV stage to maintain the PICA's 8 bits of precision
            self.out += &format!(";\nvec3 color_output_{} = byteround(", index);
            self.append_color_combiner(stage.color_op());
            self.out += ");\n";

            if stage.color_op() == TevOperation::DOT3_RGBA {
                // result of Dot3_RGBA operation is also placed to the alpha component
                self.out += &format!("float alpha_output_{0} = color_output_{0}[0];\n", index);
            } else {
                self.out += "alpha_results_1 = ";
                self.append_alpha_modifier(stage.alpha_modifier1(), stage.alpha_source1(), index);
                self.out += ";\nalpha_results_2 = ";
                self.append_alpha_modifier(stage.alpha_modifier2(), stage.alpha_source2(), index);
                self.out += ";\nalpha_results_3 = ";
                self.append_alpha_modifier(stage.alpha_modifier3(), stage.alpha_source3(), index);

                self.out += &format!(";\nfloat alpha_output_{} = byteround(", index);
                self.append_alpha_combiner(stage.alpha_op());
                self.out += ");\n";
            }

            self.out += &format!(
                "combiner_output = vec4(\
                 clamp(color_output_{} * {}.0, vec3(0.0), vec3(1.0)), \
                 clamp(alpha_output_{} * {}.0, 0.0, 1.0));\n",
                index,
                stage.get_color_multiplier(),
                index,
                stage.get_alpha_multiplier()
            );
        }

        self.out += "combiner_buffer = next_combiner_buffer;\n";
        if self.config.tev_stage_updates_combiner_buffer_color(index) {
            self.out += "next_combiner_buffer.rgb = combiner_output.rgb;\n";
        }
        if self.config.tev_stage_updates_combiner_buffer_alpha(index) {
            self.out += "next_combiner_buffer.a = combiner_output.a;\n";
        }
    }
}
