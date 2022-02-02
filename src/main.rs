use std::{
    collections::HashMap,
    fs::{read_to_string, OpenOptions},
    io::{self, Read, Write},
};

use anyhow::Context;
use clap::{arg, App, Arg};
use tree_sitter::{InputEdit, Language, Node, Parser, Point, Query, QueryCursor, Tree};
#[cfg(feature = "c")]
use tree_sitter_c::language as c_language;
#[cfg(feature = "cpp")]
use tree_sitter_cpp::language as cpp_language;
#[cfg(feature = "rust")]
use tree_sitter_rust::language as rust_language;

mod script_parser;

use script_parser::{parse, ACommandOptions, Address, Options, Script};

/// Execute query based on `query_patten` and `source_code`
fn execute_query<'a>(
    lang: Language,
    query_patten: String,
    source_code: &String,
    root_node: Node<'a>,
) -> anyhow::Result<HashMap<String, Vec<Node<'a>>>> {
    let mut cursor = QueryCursor::new();
    let query = Query::new(lang, &query_patten).context("Failed to parse query")?;
    let capture_names = query.capture_names();
    let mut node_map: HashMap<String, Vec<Node>> = HashMap::new();
    for m in cursor.matches(&query, root_node, source_code.as_bytes()) {
        for c in m.captures {
            let matched_node = c.node;
            // Insert capture name and position into table
            let entry = node_map
                .entry(
                    capture_names
                        .get(c.index as usize)
                        .context(format!("cannot get name from index, {}", c.index))?
                        .to_string(),
                )
                .or_insert(vec![]);
            entry.push(matched_node);
        }
    }
    Ok(node_map)
}

/// Calculate edit position
fn calculate_edit(node: &Node, value: &String) -> InputEdit {
    let start_byte = node.start_byte();
    let new_end_byte = start_byte + value.len();
    let start_position = node.start_position();
    let new_end_position = Point::new(start_position.row, start_position.column + value.len());
    InputEdit {
        start_byte,
        old_end_byte: node.end_byte(),
        new_end_byte,
        start_position,
        old_end_position: node.end_position(),
        new_end_position,
    }
}

/// Replace source code with `replace_table`
fn replace_source(
    tree: Tree,
    parser: &mut Parser,
    node_map: &mut HashMap<String, Vec<Node>>,
    source_code: &mut String,
    replace_table: HashMap<String, String>,
) -> anyhow::Result<()> {
    let mut edit_tree = tree;
    let mut all_edit: Vec<InputEdit> = Vec::new();
    for (name, value) in replace_table.iter() {
        let nodes = node_map
            .get_mut(name)
            .context(format!("Cannot get name {}", name))?;
        for node in nodes.iter_mut() {
            // Edit all node to its new position
            for edit in &all_edit {
                node.edit(edit);
            }
            // Replace in source code
            // end_byte points to tail + 1
            source_code.replace_range(node.start_byte()..node.end_byte(), value);
            let input_edit = calculate_edit(node, value);
            all_edit.push(input_edit);
            // Edit and parse after modifying source code
            edit_tree.edit(&input_edit);
            edit_tree = parser
                .parse(&source_code, Some(&edit_tree))
                .context("Re-generate tree fail")?;
        }
    }
    Ok(())
}

/// Delete matched node in source code
fn delete_node(
    tree: Tree,
    parser: &mut Parser,
    node_map: &mut HashMap<String, Vec<Node>>,
    source_code: &mut String,
) -> anyhow::Result<()> {
    let mut edit_tree = tree;
    let mut all_edit: Vec<InputEdit> = Vec::new();
    let empty_str = String::from("");
    for (_, nodes) in node_map.iter_mut() {
        for node in nodes {
            for edit in &all_edit {
                node.edit(edit);
            }
            source_code.replace_range(node.start_byte()..node.end_byte(), &empty_str);
            let input_edit = calculate_edit(node, &empty_str);
            all_edit.push(input_edit);
            // Edit and parse after modifying source code
            edit_tree.edit(&input_edit);
            edit_tree = parser
                .parse(&source_code, Some(&edit_tree))
                .context("Re-generate tree fail")?;
        }
    }
    Ok(())
}

fn append_content(
    tree: Tree,
    parser: &mut Parser,
    node_map: &mut HashMap<String, Vec<Node>>,
    source_code: &mut String,
    content: String,
    is_insert: bool,
) -> anyhow::Result<()> {
    let mut edit_tree = tree;
    let mut all_edit = vec![];
    for nodes in node_map.values_mut() {
        for node in nodes {
            for edit in &all_edit {
                node.edit(edit);
            }
            // Modify position depends on insert or append data
            let (mod_start_byte, mod_start_pos) = if is_insert == true {
                (node.start_byte(), node.start_position())
            } else {
                (node.end_byte(), node.end_position())
            };
            source_code.insert_str(mod_start_byte, &content);
            let input_edit = InputEdit {
                start_byte: mod_start_byte,
                old_end_byte: mod_start_byte,
                new_end_byte: mod_start_byte + content.len(),
                start_position: mod_start_pos,
                old_end_position: mod_start_pos,
                new_end_position: Point {
                    row: mod_start_pos.row,
                    column: mod_start_pos.row + content.len(),
                },
            };
            all_edit.push(input_edit);
            edit_tree.edit(&input_edit);
            edit_tree = parser
                .parse(&source_code, Some(&edit_tree))
                .context("Re-generate tree fail")?;
        }
    }
    Ok(())
}

/// Print matched node
fn print_node(
    node_map: &mut HashMap<String, Vec<Node>>,
    source_code: &mut String,
) -> anyhow::Result<()> {
    let mut print_content: Vec<&str> = vec![];
    for nodes in node_map.values() {
        for node in nodes {
            let matched = source_code
                .get(node.start_byte()..node.end_byte())
                .context("get range fail")?;
            print_content.push(matched);
        }
    }
    *source_code = print_content.join("\n");
    Ok(())
}

/// Get script's ast and execute command in script
fn execute_script(
    lang: Language,
    parser: &mut Parser,
    script: Script,
    source_code: &mut String,
) -> anyhow::Result<()> {
    // Parse code
    let tree = parser
        .parse(source_code.clone(), None)
        .context("Failed to parse source code")?;
    let root_node = tree.root_node();
    match script.command {
        's' => {
            let options = match script.options {
                Some(Options::S(options)) => options,
                _ => return Err(anyhow::format_err!("missing `s` command's options")),
            };
            let mut node_map = execute_query(lang, options.pattern, &source_code, root_node)?;
            // Re-generate syntax tree
            let mut replace_table: HashMap<String, String> = HashMap::new();
            let placeholder = options.placeholder.unwrap_or(String::from("tbr"));
            replace_table.insert(placeholder, options.replace);
            replace_source(
                tree.clone(),
                parser,
                &mut node_map,
                source_code,
                replace_table,
            )?;
        }
        cmd @ ('d' | 'p' | 'a' | 'i') => {
            let pattern = match script.address {
                Some(Address::Pattern(p)) => p,
                _ => return Err(anyhow::format_err!("missing pattern in {} command", cmd)),
            };
            let mut node_map = execute_query(lang, pattern, &source_code, root_node)?;
            match cmd {
                'd' => delete_node(tree.clone(), parser, &mut node_map, source_code)?,
                'p' => print_node(&mut node_map, source_code)?,
                'a' | 'i' => {
                    let content = match script.options {
                        Some(Options::A(ACommandOptions { content })) => content,
                        _ => return Err(anyhow::format_err!("missing content in a command")),
                    };
                    append_content(
                        tree.clone(),
                        parser,
                        &mut node_map,
                        source_code,
                        content,
                        if cmd == 'a' { false } else { true },
                    )?
                }
                _ => (),
            }
        }
        _ => todo!("More command"),
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    // TODO add more options to compatible with sed
    let app = App::new("tree-sed")
        .arg(arg!([SCRIPT]).required(true))
        .arg(arg!([FILE]))
        .arg(
            Arg::new("in-place")
                .short('i')
                .long("in-place")
                .help("edit files in place"),
        )
        .arg(arg!(--language ... "set language").default_value("c"));
    let matches = app.get_matches();
    let script = matches
        .value_of("SCRIPT")
        .context("Missing [SCRIPT] argument")?;
    let script = parse(script).context("[SCRIPT] format error")?;
    let mut source_code = match matches.value_of("FILE") {
        Some(file_name) => read_to_string(file_name)?,
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    // Init Parser
    let mut parser = Parser::new();
    let lang = match matches.value_of("language") {
        #[cfg(feature = "c")]
        Some("c") => c_language(),
        #[cfg(feature = "cpp")]
        Some("cpp") => cpp_language(),
        #[cfg(feature = "rust")]
        Some("rust") => rust_language(),
        Some(other) => return Err(anyhow::format_err!("you don't have {} parser", other)),
        None => return Err(anyhow::format_err!("missing `--language` argument")),
    };
    parser.set_language(lang)?;
    // Start executing command
    execute_script(lang, &mut parser, script, &mut source_code)?;
    match matches.occurrences_of("in-place") {
        0 => println!("{}", source_code),
        1 => {
            // TODO in-place write
            let filename = match matches.value_of("FILE") {
                Some(name) => name,
                None => return Err(anyhow::format_err!("[FILE] not exist")),
            };
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(filename)?;
            file.write(source_code.as_bytes())?;
        }
        _ => (),
    }
    Ok(())
}
