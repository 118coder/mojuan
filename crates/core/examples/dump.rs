//! 调试用：打印 Markdown 解析后的结构化结果。
use mojuan_core::doc::{self, Block};

fn show(blocks: &[Block], depth: usize) {
    let pad = "  ".repeat(depth);
    for b in blocks {
        match b {
            Block::Heading { level, plain, id, .. } => println!("{pad}H{level} id={id} text={plain:?}"),
            Block::Para(runs) => {
                let s: String = runs.iter().map(|r| r.text.as_str()).collect();
                println!("{pad}P {s:?}");
            }
            Block::Code { lang, lines } => println!("{pad}CODE lang={lang} lines={}", lines.len()),
            Block::Quote(inner) => {
                println!("{pad}QUOTE");
                show(inner, depth + 1);
            }
            Block::List { ordered, start, items } => {
                println!("{pad}LIST ordered={ordered} start={start} items={}", items.len());
                for it in items {
                    println!("{pad}  ITEM task={:?}", it.task);
                    show(&it.blocks, depth + 2);
                }
            }
            Block::Table { head, rows, aligns } => {
                println!("{pad}TABLE cols={} aligns={aligns:?} rows={}", head.len(), rows.len());
                println!("{pad}  HEAD {:?}", head.iter().map(|c| c.iter().map(|r| r.text.clone()).collect::<Vec<_>>()).collect::<Vec<_>>());
                for r in rows {
                    println!("{pad}  ROW {:?}", r.iter().map(|c| c.iter().map(|x| x.text.clone()).collect::<Vec<_>>()).collect::<Vec<_>>());
                }
            }
            Block::Rule => println!("{pad}HR"),
            Block::Image { src, path, alt, .. } => println!("{pad}IMG {src} -> {path:?} alt={alt:?}"),
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let base = std::path::Path::new(&path).parent().map(|p| p.to_path_buf());
    let d = doc::parse(&text, base.as_deref());
    show(&d.blocks, 0);
    println!("--- toc {} stats {:?}", d.toc.len(), d.stats);
}
