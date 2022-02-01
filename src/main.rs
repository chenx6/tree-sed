use std::{
    collections::HashMap,
    fs::{read_to_string, OpenOptions},
    io::{self, Read, Write},
};

use anyhow::Context;
use clap::{arg, App, Arg};
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, Tree};
use tree_sitter_c::language;

mod script_parser;

use script_parser::parse;

/// Execute query based on `query_patten` and `source_code`
fn execute_query<'a>(
    query_patten: String,
    source_code: &String,
    root_node: Node<'a>,
) -> anyhow::Result<HashMap<String, Vec<Node<'a>>>> {
    let mut cursor = QueryCursor::new();
    let query = Query::new(language(), &query_patten).context("Failed to parse query")?;
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
            // Calculate its edit position
            let start_byte = node.start_byte();
            let new_end_byte = start_byte + value.len();
            let start_position = node.start_position();
            let new_end_position =
                Point::new(start_position.row, start_position.column + value.len());
            let input_edit = InputEdit {
                start_byte,
                old_end_byte: node.end_byte(),
                new_end_byte,
                start_position,
                old_end_position: node.end_position(),
                new_end_position,
            };
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
        .arg(arg!(--language ... "set language"));
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
    parser.set_language(language())?;
    // Parse code
    let tree = parser
        .parse(source_code.clone(), None)
        .context("Failed to parse source code")?;
    let root_node = tree.root_node();
    // Start query
    let patten = script.patten.context("Missing query patten in [SCRIPT]")?;
    let mut node_map = execute_query(patten, &source_code, root_node)?;
    // Re-generate syntax tree
    let mut replace_table: HashMap<String, String> = HashMap::new();
    let placeholder = script.placeholder.unwrap_or(String::from("tbr"));
    let replace = script.replace.context("Missing replace in [SCRIPT]")?;
    replace_table.insert(placeholder, replace);
    replace_source(
        tree.clone(),
        &mut parser,
        &mut node_map,
        &mut source_code,
        replace_table,
    )?;
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