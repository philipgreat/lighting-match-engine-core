
use crate::number_tool::parse_human_readable_u32;

pub fn get_config() -> Result<(String, u16, u32), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut instance_name = None;
    let mut product_id = None;
    let mut test_order_book_size_str = None;

    // Command Line Arguments Parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                if i + 1 < args.len() {
                    instance_name = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--tag" => {
                if i + 1 < args.len() {
                    instance_name = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--prodid" => {
                if i + 1 < args.len() {
                    product_id = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            
            "--test-order-book-size" => {
                if i + 1 < args.len() {
                    test_order_book_size_str = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // 1. Instance Name (Tag)
    let tag_string = instance_name
        .or_else(|| std::env::var("INST_NAME").ok())
        .unwrap_or_else(|| "DEFAULT".to_string());

    if tag_string.len() > 16 {
        return Err(format!(
            "Instance tag '{}' exceeds maximum length of 16 characters.",
            tag_string
        ));
    }

    // 2. Product ID
    let prod_id_str = product_id.ok_or_else(|| {
        "Missing required argument: --prodid. Also check env var PROD_ID.".to_string()
    })?;
    let prod_id: u16 = prod_id_str.parse().map_err(|_| {
        format!(
            "Invalid product ID format: '{}'. Must be a valid u16.",
            prod_id_str
        )
    })?;

    // 3. Multicast Addresses
    

    let size_str: &str = test_order_book_size_str
        .as_deref() // Converts Option<String> to Option<&str>
        .unwrap_or("0"); // If None, use "0" as the default &str

    let test_order_book_size: u32 = parse_human_readable_u32(size_str).unwrap_or_else(|e| {
        eprintln!("Error parsing size '{}': {}", size_str, e);
        // Fallback u32 value if the parsing of the string (even the default "0") fails
        0
    });

    Ok((
        tag_string,
        prod_id,
        test_order_book_size,
    ))
}