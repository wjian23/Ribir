use super::*;
use crate::pipe::{InnerPipe, PipeKeyWidget, PipeWidget, ValuePipe};

pub struct MultiPair<'a> {
  pub(crate) parent: Widget<'static>,
  pub(crate) children: Vec<Widget<'a>>,
}

impl<'a> MultiPair<'a> {
  #[inline]
  pub fn new<const N: usize, const M: usize>(
    parent: impl MultiChild, children: impl IntoChildMulti<'a, N, M>,
  ) -> Self {
    let children = children.into_child_multi().collect();
    Self { parent: parent.into_widget(), children }
  }

  pub fn with_child<'b, 'c, const N: usize, const M: usize>(
    self, child: impl IntoChildMulti<'b, N, M>,
  ) -> MultiPair<'c>
  where
    'a: 'c,
    'b: 'c,
  {
    let mut children: Vec<Widget<'c>> = self.children;
    for c in child.into_child_multi() {
      children.push(c);
    }

    MultiPair { parent: self.parent, children }
  }
}

impl<'w, const M: usize, W: IntoWidget<'w, M>> IntoChildMulti<'w, 0, M> for W {
  fn into_child_multi(self) -> impl Iterator<Item = Widget<'w>> {
    std::iter::once(self.into_widget())
  }
}

impl<'w, I, const M: usize> IntoChildMulti<'w, 1, M> for I
where
  I: IntoIterator + 'w,
  I::Item: IntoWidget<'w, M>,
{
  fn into_child_multi(self) -> impl Iterator<Item = Widget<'w>> {
    self.into_iter().map(|w| w.into_widget())
  }
}

impl<'w, C, const M: usize, I, W> IntoChildMulti<'w, 2, M> for C
where
  C: InnerPipe,
  C::Value: FnOnce() -> I,
  I: IntoIterator<Item = W>,
  W: PipeKeyWidget<'w, M>,
{
  fn into_child_multi(self) -> impl Iterator<Item = Widget<'w>> { self.build_multi().into_iter() }
}

impl<T> MultiChild for T
where
  T: StateReader<Value: MultiChild> + IntoWidget<'static, RENDER>,
{
  type Target<'c> = MultiPair<'c>;
  fn with_child<'c, const N: usize, const M: usize>(
    self, child: impl IntoChildMulti<'c, N, M>,
  ) -> MultiPair<'c> {
    MultiPair::new(self, child)
  }

  fn into_parent(self: Box<Self>) -> Widget<'static> { (*self).into_widget() }
}

macro_rules! impl_pipe_methods {
  () => {
    type Target<'c> = MultiPair<'c>;

    fn with_child<'c, const N: usize, const M: usize>(
      self, child: impl IntoChildMulti<'c, N, M>,
    ) -> MultiPair<'c> {
      MultiPair { parent: self.into_parent_widget(), children: child.into_child_multi().collect() }
    }

    fn into_parent(self: Box<Self>) -> Widget<'static> { self.into_parent_widget() }
  };
}

impl<S, F, W> MultiChild for MapPipe<W, S, F>
where
  Self: InnerPipe<Value = W>,
  W: PipeWidget<RENDER>,
  W::Widget: MultiChild + 'static,
{
  impl_pipe_methods!();
}

impl<S, F, W> MultiChild for FinalChain<W, S, F>
where
  Self: InnerPipe<Value = W>,
  W: PipeWidget<RENDER>,
  W::Widget: MultiChild + 'static,
{
  impl_pipe_methods!();
}

impl<W> MultiChild for Box<dyn Pipe<Value = W>>
where
  W: PipeWidget<RENDER> + 'static,
  W::Widget: MultiChild + 'static,
{
  impl_pipe_methods!();
}

impl<P: MultiChild> MultiChild for FatObj<P> {
  type Target<'c> = FatObj<MultiPair<'c>>;
  fn with_child<'c, const N: usize, const M: usize>(
    self, child: impl IntoChildMulti<'c, N, M>,
  ) -> FatObj<MultiPair<'c>> {
    self.map(move |p| MultiPair::new(p, child))
  }

  fn into_parent(self: Box<Self>) -> Widget<'static> {
    let this = *self;
    if !this.has_class() {
      this.into_widget()
    } else {
      panic!("A FatObj should not have a class attribute when acting as a single parent")
    }
  }
}

impl<'a> FatObj<MultiPair<'a>> {
  pub fn with_child<'b, 'c, const N: usize, const M: usize>(
    self, child: impl IntoChildMulti<'b, N, M>,
  ) -> FatObj<MultiPair<'c>>
  where
    'a: 'c,
    'b: 'c,
  {
    self.map(move |p| p.with_child(child))
  }
}

impl MultiChild for Box<dyn MultiChild> {
  type Target<'c> = MultiPair<'c>;

  fn with_child<'c, const N: usize, const M: usize>(
    self, child: impl IntoChildMulti<'c, N, M>,
  ) -> MultiPair<'c> {
    MultiPair::new(self, child)
  }

  fn into_parent(self: Box<Self>) -> Widget<'static> { todo!() }
}

impl<'w> IntoWidget<'w, RENDER> for MultiPair<'w> {
  fn into_widget(self) -> Widget<'w> {
    let MultiPair { parent, children } = self;
    Widget::new(parent, children)
  }
}

pub type PipeVec<T> = Box<dyn Pipe<Value = Box<dyn Iterator<Item = T>>>>;
pub type PipeKeyVec<T> = Box<dyn Pipe<Value = Box<dyn Iterator<Item = (Option<Key>, T)>>>>;

macro_rules! impl_pipe_vec_with_child_static {
  ($is_writer:expr, $tml_num:expr, $arg_num:expr) => {
    impl<'w, T, W: 'static, const M: usize>
      ComposeWithChild<'w, T, $is_writer, $tml_num, $arg_num, M> for PipeVecBuilder<W>
    where
      VecBuilder<W>:
        ComposeWithChild<'w, T, $is_writer, $tml_num, $arg_num, M, Target = VecBuilder<W>>,
    {
      type Target = Self;

      fn with_child(self, child: T) -> Self::Target {
        let v = self
          .inner
          .unwrap_or(InnerPipeVecBuilder::Vec(Vec::builder()));
        let v = match v {
          InnerPipeVecBuilder::Vec(v) => InnerPipeVecBuilder::Vec(v.with_child(child)),
          InnerPipeVecBuilder::Pipe(_) => panic!("PipeVec can't have pipe value and other mix!"),
        };

        PipeVecBuilder { inner: Some(v) }
      }
    }
  };

  ($is_writer:expr, $tml_num:expr) => {
    impl<'w, T, W: 'static, const N: usize, const M: usize>
      ComposeWithChild<'w, T, $is_writer, $tml_num, N, M> for PipeVecBuilder<W>
    where
      VecBuilder<W>: ComposeWithChild<'w, T, $is_writer, $tml_num, N, M, Target = VecBuilder<W>>,
    {
      type Target = Self;

      fn with_child(self, child: T) -> Self::Target {
        let v = self
          .inner
          .unwrap_or(InnerPipeVecBuilder::Vec(Vec::builder()));
        let v = match v {
          InnerPipeVecBuilder::Vec(v) => InnerPipeVecBuilder::Vec(v.with_child(child)),
          InnerPipeVecBuilder::Pipe(_) => panic!("PipeVec can't have pipe value and other mix!"),
        };

        PipeVecBuilder { inner: Some(v) }
      }
    }
  };
}

impl_pipe_vec_with_child_static!(false, 1, 0);
impl_pipe_vec_with_child_static!(false, 1, 1);
impl_pipe_vec_with_child_static!(false, 1, 2);
impl_pipe_vec_with_child_static!(false, 2);

impl<'w, T, W: 'static, D, const M: usize> ComposeWithChild<'w, T, false, 1, 3, M>
  for PipeVecBuilder<W>
where
  T: Pipe<Value = D>,
  D: IntoIterator + 'static,
  W: ComposeChildFrom<D::Item, M>,
{
  type Target = Self;

  fn with_child(self, child: T) -> Self::Target {
    assert!(self.inner.is_none());

    PipeVecBuilder {
      inner: Some(InnerPipeVecBuilder::Pipe(Box::new(child.map(|v| {
        Box::new(v.into_iter().map(W::compose_child_from)) as Box<dyn Iterator<Item = W>>
      })))),
    }
  }
}

impl<'w, T, W: 'static, C, const M: usize> ComposeWithChild<'w, T, false, 1, 4, M>
  for PipeVecBuilder<W>
where
  T: Pipe<Value = C>,
  C: IntoChildCompose<Vec<W>, M>,
{
  type Target = Self;

  fn with_child(self, child: T) -> Self::Target {
    assert!(self.inner.is_none());

    PipeVecBuilder {
      inner: Some(InnerPipeVecBuilder::Pipe(Box::new(
        child.map(|v| Box::new(v.into_child_compose().into_iter()) as Box<dyn Iterator<Item = W>>),
      ))),
    }
  }
}

impl<T> ChildOfCompose for PipeVec<T> {}

impl<T: 'static> ComposeChildFrom<PipeVecBuilder<T>, 1> for PipeVec<T> {
  #[inline]
  fn compose_child_from(from: PipeVecBuilder<T>) -> Self { from.build_tml() }
}

enum InnerPipeVecBuilder<V> {
  Pipe(Box<dyn Pipe<Value = Box<dyn Iterator<Item = V>>>>),
  Vec(VecBuilder<V>),
}

impl<V: 'static> InnerPipeVecBuilder<V> {
  fn into_pipe(self) -> Box<dyn Pipe<Value = Box<dyn Iterator<Item = V>>>> {
    match self {
      InnerPipeVecBuilder::Pipe(p) => p,
      InnerPipeVecBuilder::Vec(v) => {
        Box::new(ValuePipe::new(Box::new(v.build_tml().into_iter()) as Box<dyn Iterator<Item = V>>))
      }
    }
  }
}

pub struct PipeVecBuilder<T> {
  inner: Option<InnerPipeVecBuilder<T>>,
}

impl<T: 'static> TemplateBuilder for PipeVecBuilder<T> {
  type Target = PipeVec<T>;
  fn build_tml(self) -> Self::Target { self.inner.unwrap().into_pipe() }
}

impl<T: 'static> Template for PipeVec<T> {
  type Builder = PipeVecBuilder<T>;
  fn builder() -> Self::Builder { PipeVecBuilder { inner: None } }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::test_helper::MockMulti;

  #[test]
  fn compile_dyn_key_vec() {
    #[derive(Declare)]
    struct Multi {}

    impl ComposeChild<'static> for Multi {
      type Child = PipeKeyVec<Widget<'static>>;
      fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
        unimplemented!()
      }
    }

    let _ = fn_widget! {
      let v = PipeKeyVec::<Widget<'static>>::builder();
      v.with_child((0..3).map(|i|
        @KeyWidget {
          key: Some(Key::Number(i as isize)),
          @ { @Void {}.into_widget() }
        }
      ));

      let cnt = Stateful::new(3);
      @MockMulti {
        @Multi {
          @Void {}
          @Void {}
        }
        @Multi {
          @KeyWidget{
            key: None,
            @Void {}
          }
          @KeyWidget{
            key: None,
            @Void {}
          }
        }
        @Multi {
          @{
            let w = pipe!(*$cnt)
              .map(move |cnt| {
                @KeyVec::<Widget<'static>> {
                  @ { (0..cnt).map(|_| @KeyWidget {key: None, @Void {} }) }
                }
            });
            w
          }
        }

        @Multi {
          @{
            pipe!(*$cnt)
              .map(move |cnt| {
                let it = (0..cnt).map(|i|
                @KeyWidget {
                  key: Some(Key::Number(i as isize)),
                  @ { @Void {}.into_widget() }
                }
              );
              it
            })
          }
        }
      }
    };
  }

  #[test]
  fn compile_dyn_vec() {
    #[derive(Declare)]
    struct Multi {}

    impl ComposeChild<'static> for Multi {
      type Child = PipeVec<Widget<'static>>;
      fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
        unimplemented!()
      }
    }

    let _ = fn_widget! {
      let cnt = Stateful::new(3);

      @MockMulti {
        @Multi {
          @Void {}
          @Void {}
        }
        @Multi {
          @{
            pipe!(*$cnt)
              .map(move |cnt| (0..cnt).map(|_| @Void {}))
          }
        }
      }
    };
  }

  #[test]
  fn compile_dyn_vec_of_template() {
    #[allow(dead_code)]
    #[derive(Template)]
    struct TmlEmbed {
      text: TextInit,
    }
    #[allow(dead_code)]
    #[derive(Template)]
    enum Tml {
      Widget(Widget<'static>),
      Embed(TmlEmbed),
    }

    #[derive(Declare)]
    struct Multi {}

    impl ComposeChild<'static> for Multi {
      type Child = PipeVec<Tml>;
      fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
        unimplemented!()
      }
    }
    let _ = fn_widget! {
      @Multi {
        @ TmlEmbed { @ { "123"} }
        @ TmlEmbed { @ { "123"} }
      }
    };
  }
}
