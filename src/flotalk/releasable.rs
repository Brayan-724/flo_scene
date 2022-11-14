use super::context::*;
use super::reference::*;
use super::value::*;

use std::ops::{Deref};
use std::sync::*;

///
/// Trait implemented by types that can be released in a context
///
pub trait TalkReleasable {
    ///
    /// Releases this item within the specified context
    ///
    fn release_in_context(self, context: &TalkContext);
}

///
/// Trait implemented by types that can be cloned in a context
///
pub trait TalkCloneable {
    ///
    /// Releases this item within the specified context
    ///
    fn clone_in_context(&self, context: &TalkContext) -> Self;
}

///
/// A value that will be released when dropped
///
pub struct TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    context:    &'a TalkContext,
    value:      Option<TReleasable>
}

impl<'a, TReleasable> TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    ///
    /// Creates a new TalkOwned object
    ///
    #[inline]
    pub fn new(value: TReleasable, context: &'a TalkContext) -> TalkOwned<'a, TReleasable> {
        TalkOwned {
            context:    context, 
            value:      Some(value),
        }
    }
}

impl<'a, TReleasable> Drop for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    #[inline]
    fn drop(&mut self) {
        self.value.take().unwrap().release_in_context(self.context);
    }
}

impl<'a, TReleasable> Clone for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable + TalkCloneable
{
    fn clone(&self) -> Self {
        match &self.value {
            Some(value) => TalkOwned {
                context:    self.context,
                value:      Some(value.clone_in_context(self.context)),
            },
            None        => unreachable!()
        }
    }
}

impl<'a, TReleasable> Deref for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    type Target = TReleasable;

    #[inline]
    fn deref(&self) -> &TReleasable {
        match &self.value {
            Some(value) => value,
            None        => unreachable!()
        }
    }
}

impl TalkReleasable for TalkValue {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.remove_reference(context);
    }
}

impl TalkCloneable for TalkValue {
    ///
    /// Creates a copy of this value in the specified context
    ///
    /// This will copy this value and increase its reference count
    ///
    #[inline]
    fn clone_in_context(&self, context: &TalkContext) -> Self {
        use TalkValue::*;

        match self {
            Nil                     => Nil,
            Reference(reference)    => Reference(reference.clone_in_context(context)),
            Bool(boolean)           => Bool(*boolean),
            Int(int)                => Int(*int),
            Float(float)            => Float(*float),
            String(string)          => String(Arc::clone(string)),
            Character(character)    => Character(*character),
            Symbol(symbol)          => Symbol(*symbol),
            Selector(symbol)        => Selector(*symbol),
            Array(array)            => Array(array.iter().map(|val| val.clone_in_context(context)).collect()),
            Error(error)            => Error(error.clone()),
        }
    }
}

impl TalkReleasable for TalkReference {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.remove_reference(context);
    }
}

impl TalkCloneable for TalkReference {
    ///
    /// This will create a copy of this reference and increase its reference count
    ///
    #[inline]
    fn clone_in_context(&self, context: &TalkContext) -> TalkReference {
        let clone = TalkReference(self.0, self.1);
        if let Some(callbacks) = context.get_callbacks(self.0) {
            callbacks.add_reference(self.1);
        }
        clone
    }
}

impl<TIntoIter> TalkReleasable for TIntoIter
where
    TIntoIter:          IntoIterator,
    TIntoIter::Item:    TalkReleasable,
{
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.into_iter().for_each(|item| item.release_in_context(context));
    }
}
