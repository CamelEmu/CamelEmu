typed_instructions! {
    op_convert_f_to_u(operand) => ConvertFToU;
    op_convert_f_to_s(operand) => ConvertFToS;
    op_convert_s_to_f(operand) => ConvertSToF;
    op_convert_u_to_f(operand) => ConvertUToF;
    op_u_convert(operand) => UConvert;
    op_s_convert(operand) => SConvert;
    op_f_convert(operand) => FConvert;
    op_quantize_to_f16(operand) => QuantizeToF16;
    op_bitcast(operand) => Bitcast;
}
