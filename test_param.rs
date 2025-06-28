use tilt_ast::*;
use tilt_ir::lower_program;

fn main() {
    let helper_func = FunctionDef {
        name: "add_one",
        params: vec![TypedIdentifier {
            name: "x",
            ty: Type::I32,
        }],
        return_type: Type::I32,
        blocks: vec![Block {
            label: "entry",
            instructions: vec![],
            terminator: Terminator::Ret(Some(Value::Variable("x"))),
        }],
    };

    let ast = Program {
        items: vec![TopLevelItem::Function(helper_func)],
    };

    match lower_program(&ast) {
        Ok(ir) => {
            println!("SUCCESS: {:?}", ir);
        }
        Err(errors) => {
            println!("ERRORS:");
            for error in errors {
                println!("  {}", error);
            }
        }
    }
}
