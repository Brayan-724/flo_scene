use super::context::*;
use super::error::*;
use super::message::*;
use super::number::*;
use super::reference::*;
use super::releasable::*;
use super::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

static VALUE_MSG: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "value".into());
static VALUE_COLON_MSG: Lazy<TalkMessageSignatureId>  = Lazy::new(|| "value:".into());

impl TalkMessageType for () {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        if let TalkMessage::Unary(_any_message) = &*message {
            Ok(())
        } else {
            Err(TalkError::MessageNotSupported(message.signature_id()))
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::Unary(*VALUE_MSG), context)
    }
}

impl TalkMessageType for TalkReference {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    /// Note: the reference must be released by the caller
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        let signature = message.signature_id();

        if let TalkMessage::WithArguments(_, mut args) = message.leak() {
            if args.len() == 1 {
                match args[0].take() {
                    TalkValue::Reference(reference) => Ok(reference),
                    _                               => {
                        args.release_in_context(context);
                        Err(TalkError::NotAReference)
                    },
                }
            } else {
                args.release_in_context(context);
                Err(TalkError::MessageNotSupported(signature))
            }
        } else {
            Err(TalkError::MessageNotSupported(signature))
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Reference(self.clone_in_context(context))]), context)
    }
}

impl TalkMessageType for TalkValue {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    /// Note: the reference must be released by the caller
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        let signature = message.signature_id();

        if let TalkMessage::WithArguments(_, mut args) = message.leak() {
            if args.len() == 1 {
                Ok(args[0].take())
            } else {
                args.release_in_context(context);
                Err(TalkError::MessageNotSupported(signature))
            }
        } else {
            Err(TalkError::MessageNotSupported(signature))
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![self.clone_in_context(context)]), context)
    }
}

#[inline]
fn read_argument(msg: &TalkMessage) -> Result<&TalkValue, TalkError> {
    if let TalkMessage::WithArguments(_, args) = msg {
        if args.len() == 1 {
            Ok(&args[0])
        } else {
            Err(TalkError::MessageNotSupported(msg.signature_id()))
        }
    } else {
        Err(TalkError::MessageNotSupported(msg.signature_id()))
    }
}

impl TalkMessageType for bool {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Bool(val)    => Ok(*val),
            _                       => Err(TalkError::NotABoolean),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Bool(*self)]), context)
    }
}

impl TalkMessageType for i32 {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            _                       => Err(TalkError::NotAnInteger),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Int(*self as _)]), context)
    }
}

impl TalkMessageType for i64 {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            _                       => Err(TalkError::NotAnInteger),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Int(*self as _)]), context)
    }
}

impl TalkMessageType for f32 {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            _                       => Err(TalkError::NotAFloat),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Float(*self as _)]), context)
    }
}

impl TalkMessageType for f64 {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            _                       => Err(TalkError::NotAFloat),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Float(*self as _)]), context)
    }
}

impl TalkMessageType for TalkNumber {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Int(val)     => Ok(TalkNumber::Int(*val)),
            TalkValue::Float(val)   => Ok(TalkNumber::Float(*val)),
            _                       => Err(TalkError::NotANumber),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        match self {
            TalkNumber::Int(val)    => TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Int(*val)]), context),
            TalkNumber::Float(val)  => TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Float(*val)]), context),
        }
    }
}

impl TalkMessageType for String {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::String(val)  => Ok((**val).clone()),
            _                       => Err(TalkError::NotAString),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::String(Arc::new(self.clone()))]), context)
    }
}

impl TalkMessageType for Arc<String> {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::String(val)  => Ok(val.clone()),
            _                       => Err(TalkError::NotAString),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::String(self.clone())]), context)
    }
}

impl TalkMessageType for char {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Character(val)   => Ok(*val),
            _                           => Err(TalkError::NotACharacter),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Character(*self)]), context)
    }
}

impl TalkMessageType for TalkError {
    fn supports_message(id: TalkMessageSignatureId) -> bool {
        id == *VALUE_COLON_MSG
    }

    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        match read_argument(&*message)? {
            TalkValue::Error(val)   => Ok(val.clone()),
            _                       => Err(TalkError::NotAnError),
        }
    }

    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Error(self.clone())]), context)
    }
}
