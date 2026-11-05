# Fixture

A paragraph with **bold**, *italic*, ~~strikethrough~~, `inline code`
and a [link to example](https://example.com). An image follows:
![diagram](diagram.png).

## Lists

- first item
- second item with enough words to wrap around the render width and prove
  the hanging indent lines up
  - nested item
- [x] finished task
- [ ] open task

1. ordered one
2. ordered two

## Code

```rust
fn main() {
    println!("hello");
}
```

## Quote

> quoted text that also runs long enough to wrap inside the gutter
> second quoted line

> > nested quote

## Table

| name | value |
|------|-------|
| alpha | 1 |
| beta | 22 |

---

终端渲染测试，宽度必须对齐。
