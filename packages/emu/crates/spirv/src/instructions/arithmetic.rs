typed_instructions! {
    op_s_negate(operand) => SNegate;
    op_f_negate(operand) => FNegate;
    op_i_add(operand_1, operand_2) => IAdd;
    op_f_add(operand_1, operand_2) => FAdd;
    op_i_sub(operand_1, operand_2) => ISub;
    op_f_sub(operand_1, operand_2) => FSub;
    op_i_mul(operand_1, operand_2) => IMul;
    op_f_mul(operand_1, operand_2) => FMul;
    op_u_div(operand_1, operand_2) => UDiv;
    op_s_div(operand_1, operand_2) => SDiv;
    op_f_div(operand_1, operand_2) => FDiv;
    op_u_mod(operand_1, operand_2) => UMod;
    op_s_mod(operand_1, operand_2) => SMod;
    op_f_mod(operand_1, operand_2) => FMod;
    op_s_rem(operand_1, operand_2) => SRem;
    op_f_rem(operand_1, operand_2) => FRem;
    op_i_add_carry(operand_1, operand_2) => IAddCarry;
    op_vector_times_scalar(operand_1, operand_2) => VectorTimesScalar;
    op_dot(operand_1, operand_2) => Dot;
}
