pub struct Menu<'a, M, C> {
    pub rows: &'a [Row<'a, M, C>],
}

pub struct Row<'a, M, C> {
    pub text: &'a [u8],
    pub content: Content<'a, M, C>,
}

pub enum Content<'a, M, C> {
    SubMenu(Menu<'a, M, C>),
    MenuItem(Item<M, C>),
}

pub struct Item<M, C> {
    pub update: fn(model: &mut M, command: C),
    pub view: fn(model: &M) -> &[u8],
}
