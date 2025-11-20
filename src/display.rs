use alloy::{dyn_abi::DynSolValue, hex};
use alloy_json_abi::Function;
use comfy_table::{Attribute, Cell, Color, Table};

/// Formats a DynSolValue into a human-readable string.
fn format_value(value: &DynSolValue) -> String {
    match value {
        DynSolValue::Address(addr) => {
            // Format checksum address
            let add_str = format!("{:?}", addr);

            // Check for well-known address
            if addr.is_zero() {
                format!("{} (Zero Address)", add_str)
            } else {
                add_str
            }
        }
        DynSolValue::Uint(val, bits) => {
            // Format bigint with underscores
            let num_str = val.to_string();
            if num_str.len() > 6 {
                // Insert underscores every 3 digits from the right
                let formatted = num_str
                    .chars()
                    .rev()
                    .collect::<Vec<_>>()
                    .chunks(3)
                    .map(|chunk| chunk.iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("_")
                    .chars()
                    .rev()
                    .collect::<String>();
                format!("{} (uint{})", formatted, bits)
            } else {
                format!("{} (uint{})", num_str, bits)
            }
        }
        DynSolValue::Bool(b) => format!("{}", b),
        DynSolValue::Bytes(bytes) => {
            if bytes.len() <= 32 {
                format!("0x{}", hex::encode(bytes))
            } else {
                format!("0x{}... ({} bytes)", hex::encode(&bytes[..32]), bytes.len())
            }
        }
        _ => format!("{:?}", value),
    }
}

/// Displays the decoded function name and parameters in a formatted table.
pub fn display_decoded(func_name: &str, params: &[DynSolValue], func: &Function) {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("Parameter")
            .fg(Color::Cyan)
            .add_attribute(Attribute::Bold),
        Cell::new("Type")
            .fg(Color::Yellow)
            .add_attribute(Attribute::Bold),
        Cell::new("Value")
            .fg(Color::Green)
            .add_attribute(Attribute::Bold),
    ]);

    // Zip parameters with their types from the function ABI
    for (i, (param, input)) in params.iter().zip(&func.inputs).enumerate() {
        table.add_row(vec![
            Cell::new(if input.name.is_empty() {
                format!("param{}", i)
            } else {
                input.name.clone()
            }),
            Cell::new(input.ty.to_string()).fg(Color::Yellow),
            Cell::new(format_value(param)).fg(Color::White),
        ]);
    }

    println!("\n✅ Function: {}", func_name);
    println!("{}", table);
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, U256};

    use super::*;

    #[test]
    fn test_format_address() {
        let addr = Address::ZERO;
        let formatted = format_value(&DynSolValue::Address(addr));
        assert!(formatted.contains("Zero Address"));
    }

    #[test]
    fn test_format_uint() {
        let val = U256::from(1_000_000);
        let formatted = format_value(&DynSolValue::Uint(val, 256));
        assert!(formatted.contains("1_000_000"));
    }
}
