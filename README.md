# tree-sed

Manpulate ast tree like `sed`.

## Example

Using `sed`-like grammer to replace `puts` argument.

```bash
$ cargo run -- 's/(call_expression
  function: (identifier) @the-function
  arguments: (argument_list (_) @tbr)
  (#eq? @the-function "puts"))/"Just Monika"/' ./example/source_code.c
```

![Example](./example/Screenshot.png)

## TODO

- [ ] Implement more argument to compatible to `sed`
- [ ] Implement more sed script's function
  - [ ] `s` command: `&` and `\1 \2 ...`, `g` option
  - [x] `i`/`a` command: insert/append content
  - [x] `d` command: delete content
  - [x] `p` command: print content
- [ ] Better document

# Acknowledgement

- [tree-sitter/tree-sitter](https://github.com/tree-sitter/tree-sitter)
- [sed, a stream editor](https://www.gnu.org/software/sed/manual/sed.html)
