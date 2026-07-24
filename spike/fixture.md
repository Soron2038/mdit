# mdit Spike Fixture

Regular paragraph with **bold**, *italic*, ~~strikethrough~~ and a
[web link](https://example.com) plus `inline code`.

## Image (standard syntax, relative path)

![test image](img.png)

## Code block

```swift
struct Point {
    let x: Double
    let y: Double
    func distance(to other: Point) -> Double {
        ((x - other.x) * (x - other.x) + (y - other.y) * (y - other.y)).squareRoot()
    }
}
```

## GFM table

| Feature    | Status | Notes            |
|------------|--------|------------------|
| Read-only  | ?      | isEditable=false |
| Tables     | ?      | this one         |
| Highlight  | ?      | swift fence      |

## Task list

- [x] clone engine
- [ ] verify rendering
- regular bullet
