use std::collections::HashMap;

use koopa::ir::{*, entities};
const REGS:[&str;16] = ["x0",
          "t0","t1","t2","t3","t4","t5","t6",
          "a0","a1","a2","a3","a4","a5","a6","a7"];

/// 根据内存形式 Koopa IR 生成 RISC-V 汇编
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
      let mut reg_map: HashMap<Value, String> = HashMap::new();
      let mut next_reg: usize = 1; // 从 t0 开始, 跳过 x0
      for (&value, _) in bb_node.insts() {
        let value_data = self.dfg().value(value);
        generate_value(value, value_data, self, buf, &mut reg_map, &mut next_reg);
      }
    }
  }
}

fn alloc_reg(
  value: Value,
  reg_map: &mut HashMap<Value, String>,
  next_reg: &mut usize,
) -> String {
  let reg = REGS[*next_reg].to_string();
  *next_reg += 1;
  reg_map.insert(value, reg.clone());
  reg
}

fn ensure_reg(
  value: Value,
  func: &FunctionData,
  buf: &mut String,
  reg_map: &mut HashMap<Value, String>,
  next_reg: &mut usize,
) -> String {
  if let Some(reg) = reg_map.get(&value) {
    return reg.clone();
  }
  let value_data = func.dfg().value(value);
  match value_data.kind(){
    ValueKind::Integer(i)=>{
      if i.value()==0{
        //return x0
        return REGS[0].to_string();
      }
    }
    _=>{}
  }
  generate_value(value, value_data, func, buf, reg_map, next_reg);
  reg_map.get(&value).unwrap().clone()
}

fn generate_value(
  value: Value,
  value_data: &entities::ValueData,
  func: &FunctionData,
  buf: &mut String,
  reg_map: &mut HashMap<Value, String>,
  next_reg: &mut usize,
) {
  match value_data.kind() {
    ValueKind::Integer(i) => {
      let reg = alloc_reg(value, reg_map, next_reg);
      buf.push_str(&format!("  li {}, {}\n", reg, i.value()));
    }
    ValueKind::Binary(b) => {
      let lhs = b.lhs();
      let rhs = b.rhs();
      let lhs_reg = ensure_reg(lhs, func, buf, reg_map, next_reg);
      let rhs_reg = ensure_reg(rhs, func, buf, reg_map, next_reg);
      let reg = alloc_reg(value, reg_map, next_reg);
      match b.op() {
        BinaryOp::Add => {
          buf.push_str(&format!("  add {}, {}, {}\n", reg, lhs_reg, rhs_reg));
        }
        BinaryOp::Sub => {
          buf.push_str(&format!("  sub {}, {}, {}\n", reg, lhs_reg, rhs_reg));
        }
        BinaryOp::Eq => {
          buf.push_str(&format!("  xor {}, {}, {}\n", reg, lhs_reg, rhs_reg));
          buf.push_str(&format!("  seqz {}, {}\n", reg, reg));
        }
        _ => {}
      }
    }
    ValueKind::Return(ret) => {
      let ret_value = ret.value();
      if let Some(ret_value) = ret_value {
        let ret_reg = ensure_reg(ret_value, func, buf, reg_map, next_reg);
        if ret_reg != "a0" {
          buf.push_str(&format!("  mv a0, {}\n", ret_reg));
        }
      }
      buf.push_str("  ret\n");
    }
    _ => {}
  }
}
