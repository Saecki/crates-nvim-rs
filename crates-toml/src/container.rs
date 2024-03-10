use std::mem::ManuallyDrop;

use bumpalo::Bump;

use crate::{Asts, Ctx, MapTable, Tokens};

pub struct Toml<'a> {
    pub input: &'a str,
    pub tokens: Tokens<'a>,
    pub asts: Asts<'a>,
    pub map: MapTable<'a>,
}

/// Self contained, movable container for a parsed [`Toml`] structure.
pub struct Container {
    toml: ManuallyDrop<Toml<'static>>,
    #[allow(unused)]
    bump: &'static Bump,
}

impl Drop for Container {
    fn drop(&mut self) {
        // SAFETY: drop is only ever called once
        unsafe {
            ManuallyDrop::drop(&mut self.toml);
        }

        let ptr = self.bump as *const Bump;
        // SAFETY: `self.bump` is only ever constructed using `Box::leak` and the static reference does
        // never escape the private api. The only references to `self.bump` are in `self.toml`
        // which is explicitly dropped before.
        unsafe {
            let bump = Box::from_raw(ptr.cast_mut());
            drop(bump);
        }
    }
}

impl<'a> Container {
    pub fn parse(ctx: &mut Ctx, input: &str) -> Container {
        let bump = Box::leak(Box::new(Bump::new()));

        let input = bump.alloc_str(input);
        let tokens = ctx.lex(bump, input);
        let asts = ctx.parse(bump, &tokens);
        let map = ctx.map(bump, &asts);

        let toml = Toml {
            input,
            tokens,
            asts,
            map,
        };
        let toml = ManuallyDrop::new(toml);

        Container { toml, bump }
    }

    pub fn toml(&'a self) -> &'a Toml<'a> {
        // only give out a reference which is restricted to the container's lifetime
        &self.toml
    }
}