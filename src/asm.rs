use std::collections::HashMap;

use koopa::ir::{*, entities};

pub trait GenerateAsm {
  fn generate(&self, buf: &mut String);
}

impl GenerateAsm for Program {
  fn generate(&self, buf: &mut String) {
    buf.push_str(" .text\n");
    for &func in self.func_layout() {
      self.func(func).generate(buf);
    }
  }
}

impl GenerateAsm for FunctionData {
  fn generate(&self, buf: &mut String) {
    buf.push_str(" .global ");
    let fun_name = String::from(self.name());
    buf.push_str(&fun_name[1..]);
    buf.push_str("\n");
    buf.push_str(&fun_name[1..]);
    buf.push_str(":\n");

    for bb_node in self.layout().bbs().nodes() {
      // Pass 1: 扫描指令，用 ty().is_unit() 统计需要栈槽的数量
      let mut slot_count = 0;
      for (&value, _) in bb_node.insts() {
        let vd = self.dfg().value(value);
        if !vd.ty().is_unit() {
          match vd.kind() {
            ValueKind::Integer(_) => {} // 常量不占槽
            _ => slot_count += 1,
          }
        }
      }

      // 对齐到 16 字节
      let stack_size = if slot_count == 0 { 0 }
        else { ((slot_count * 4 + 15) / 16) * 16 };

      // Prologue
      emit_prologue(buf, stack_size);

      // Pass 2: 生成代码，延迟分配栈偏移
      let mut offset_map: HashMap<Value, i32> = HashMap::new();
      let mut next_slot: i32 = 0;
      for (&value, _) in bb_node.insts() {
        let vd = self.dfg().value(value);
        generate_inst(value, vd, self, buf, &mut offset_map, &mut next_slot, stack_size);
      }
    }
  }
}

// ==================== Prologue / Epilogue ====================

fn emit_prologue(buf: &mut String, size: i32) {
  if size == 0 { return; }
  if size <= 2048 {
    buf.push_str(&format!("  addi sp, sp, -{}\n", size));
  } else {
    buf.push_str(&format!("  li t0, -{}\n", size));
    buf.push_str("  add sp, sp, t0\n");
  }
}

fn emit_epilogue(buf: &mut String, size: i32) {
  if size == 0 { return; }
  if size <= 2048 {
    buf.push_str(&format!("  addi sp, sp, {}\n", size));
  } else {
    buf.push_str(&format!("  li t0, {}\n", size));
    buf.push_str("  add sp, sp, t0\n");
  }
}

// ==================== 延迟分配 ====================

fn get_offset(
  value: Value,
  offset_map: &mut HashMap<Value, i32>,
  next_slot: &mut i32,
) -> i32 {
  *offset_map.entry(value).or_insert_with(|| {
    let off = *next_slot * 4;
    *next_slot += 1;
    off
  })
}

// ==================== Pass 2: 代码生成 ====================

fn load_op(
  value: Value,
  func: &FunctionData,
  buf: &mut String,
  offset_map: &mut HashMap<Value, i32>,
  next_slot: &mut i32,
  temp: &str,
) -> String {
  let vd = func.dfg().value(value);
  match vd.kind() {
    ValueKind::Integer(i) => {
      if i.value() == 0 {
        "x0".to_string()
      } else {
        buf.push_str(&format!("  li {}, {}\n", temp, i.value()));
        temp.to_string()
      }
    }
    _ => {
      let offset = get_offset(value, offset_map, next_slot);
      buf.push_str(&format!("  lw {}, {}(sp)\n", temp, offset));
      temp.to_string()
    }
  }
}

fn generate_inst(
  value: Value,
  vd: &entities::ValueData,
  func: &FunctionData,
  buf: &mut String,
  offset_map: &mut HashMap<Value, i32>,
  next_slot: &mut i32,
  stack_size: i32,
) {
  match vd.kind() {
    ValueKind::Binary(b) => {
      let lhs_reg = load_op(b.lhs(), func, buf, offset_map, next_slot, "t0");
      let rhs_reg = load_op(b.rhs(), func, buf, offset_map, next_slot, "t1");

      match b.op() {
        BinaryOp::Add => buf.push_str(&format!("  add t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Sub => buf.push_str(&format!("  sub t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Mul => buf.push_str(&format!("  mul t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Div => buf.push_str(&format!("  div t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Mod => buf.push_str(&format!("  rem t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Eq => {
          buf.push_str(&format!("  xor t0, {}, {}\n", lhs_reg, rhs_reg));
          buf.push_str("  seqz t0, t0\n");
        }
        BinaryOp::NotEq => {
          buf.push_str(&format!("  xor t0, {}, {}\n", lhs_reg, rhs_reg));
          buf.push_str("  snez t0, t0\n");
        }
        BinaryOp::Lt => buf.push_str(&format!("  slt t0, {}, {}\n", lhs_reg, rhs_reg)),
        BinaryOp::Gt => buf.push_str(&format!("  slt t0, {}, {}\n", rhs_reg, lhs_reg)),
        BinaryOp::Le => {
          buf.push_str(&format!("  slt t0, {}, {}\n", rhs_reg, lhs_reg));
          buf.push_str("  xori t0, t0, 1\n");
        }
        BinaryOp::Ge => {
          buf.push_str(&format!("  slt t0, {}, {}\n", lhs_reg, rhs_reg));
          buf.push_str("  xori t0, t0, 1\n");
        }
        BinaryOp::And => {
          buf.push_str(&format!("  snez t0, {}\n", lhs_reg));
          buf.push_str(&format!("  snez t2, {}\n", rhs_reg));
          buf.push_str("  and t0, t0, t2\n");
        }
        BinaryOp::Or => {
          buf.push_str(&format!("  or t0, {}, {}\n", lhs_reg, rhs_reg));
          buf.push_str("  snez t0, t0\n");
        }
        _ => {}
      }

      let offset = get_offset(value, offset_map, next_slot);
      buf.push_str(&format!("  sw t0, {}(sp)\n", offset));
    }
    ValueKind::Return(ret) => {
      if let Some(ret_val) = ret.value() {
        let ret_reg = load_op(ret_val, func, buf, offset_map, next_slot, "a0");
        // if ret_reg != "a0" {
        //   buf.push_str(&format!("  mv a0, {}\n", ret_reg));
        // }
      }
      emit_epilogue(buf, stack_size);
      buf.push_str("  ret\n");
    }
    ValueKind::Store(s)=>{
      let v = s.value();
      let d = s.dest();
      let reg = load_op(v, func, buf, offset_map, next_slot,"t0");
      let offset = get_offset(d, offset_map, next_slot);
      buf.push_str(&format!("  sw {}, {}(sp)\n", reg, offset));
    }
    ValueKind::Load(l) =>{
      let v = l.src();
      let reg = load_op(v, func, buf, offset_map, next_slot,"t0");
      let offset = get_offset(value, offset_map, next_slot);
      buf.push_str(&format!("  sw {}, {}(sp)\n", reg, offset));
    }
    _ => {}
  }
}
