struct Point {
    x: int,
    y: int,
}

struct Rectangle {
    top_left: Point,
    bottom_right: Point,
}

fn add_points(a: Point, b: Point) -> Point {
    return Point { x: a.x + b.x, y: a.y + b.y };
}

fn main() {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 5, y: 15 };
    let result = add_points(p1, p2);

    let rect = Rectangle {
        top_left: Point { x: 0, y: 100 },
        bottom_right: Point { x: 100, y: 0 },
    };

    return result;
}