use super::expression::*;
use super::symbol::*;
use super::value::*;

use smallvec::*;

use std::fmt;
use std::sync::*;
use std::collections::{HashMap};

lazy_static! {
    /// The ID to assign to the next message signature
    static ref NEXT_SIGNATURE_ID: Mutex<usize>                                                  = Mutex::new(0);

    /// Maps between signatures and their IDs
    static ref ID_FOR_SIGNATURE: Mutex<HashMap<TalkMessageSignature, TalkMessageSignatureId>>   = Mutex::new(HashMap::new());

    /// Maps between IDs and signatures
    static ref SIGNATURE_FOR_ID: Mutex<HashMap<TalkMessageSignatureId, TalkMessageSignature>>   = Mutex::new(HashMap::new());
}

///
/// Represents a flotalk message
///
#[derive(Clone)]
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkMessageSignatureId),

    /// A message with named arguments
    WithArguments(TalkMessageSignatureId, SmallVec<[TalkValue; 4]>),
}

impl TalkMessage {
    ///
    /// Creates a unary message
    ///
    pub fn unary(symbol: impl Into<TalkSymbol>) -> TalkMessage {
        TalkMessage::Unary(TalkMessageSignature::Unary(symbol.into()).id())
    }

    ///
    /// Creates a message with arguments
    ///
    pub fn with_arguments(arguments: impl IntoIterator<Item=(impl Into<TalkSymbol>, impl Into<TalkValue>)>) -> TalkMessage {
        let mut signature_symbols   = smallvec![];
        let mut argument_values     = smallvec![];

        for (symbol, value) in arguments {
            signature_symbols.push(symbol.into());
            argument_values.push(value.into());
        }

        TalkMessage::WithArguments(TalkMessageSignature::Arguments(signature_symbols).id(), argument_values)
    }
}

///
/// A message signature describes a message
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TalkMessageSignature {
    Unary(TalkSymbol),
    Arguments(SmallVec<[TalkSymbol; 4]>),
}

///
/// A unique ID for a message signature
///
/// This is just an integer value underneath, and can be used to quickly look up a message without having to compare all the symbols individually
///
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TalkMessageSignatureId(usize);

impl TalkMessage {
    ///
    /// Converts a message to its signature
    ///
    #[inline]
    pub fn signature(&self) -> TalkMessageSignature {
        match self {
            TalkMessage::Unary(id)                  => id.to_signature(),
            TalkMessage::WithArguments(id, _args)   => id.to_signature()
        }
    }
}

impl TalkMessageSignature {
    ///
    /// Returns the ID for this signature
    ///
    pub fn id(&self) -> TalkMessageSignatureId {
        let id_for_signature = ID_FOR_SIGNATURE.lock().unwrap();

        if let Some(id) = id_for_signature.get(self) {
            // ID already defined
            *id
        } else {
            let mut id_for_signature = id_for_signature;

            // Create a new ID
            let new_id = {
                let mut next_signature_id   = NEXT_SIGNATURE_ID.lock().unwrap();
                let new_id                  = *next_signature_id;
                *next_signature_id += 1;

                new_id
            };
            let new_id = TalkMessageSignatureId(new_id);

            // Store the ID for this signature
            id_for_signature.insert(self.clone(), new_id);
            SIGNATURE_FOR_ID.lock().unwrap().insert(new_id, self.clone());

            new_id
        }
    }

    ///
    /// Creates a unary message signature
    ///
    pub fn unary(symbol: impl Into<TalkSymbol>) -> TalkMessageSignature {
        TalkMessageSignature::Unary(symbol.into())
    }

    ///
    /// Creates a message signature with arguments
    ///
    pub fn with_arguments(symbols: impl IntoIterator<Item=impl Into<TalkSymbol>>) -> TalkMessageSignature {
        TalkMessageSignature::Arguments(symbols.into_iter().map(|sym| sym.into()).collect())
    }

    ///
    /// Returns true if an argument list represents a unary list
    ///
    pub fn arguments_are_unary<'a>(args: impl IntoIterator<Item=&'a TalkArgument>) -> bool {
        let mut arg_iter = args.into_iter();

        if let Some(first) = arg_iter.next() {
            if first.value.is_none() {
                // Unary if there's a single argument with no value
                let next = arg_iter.next();

                debug_assert!(next.is_none());

                next.is_none()
            } else {
                // First argument has a value
                false
            }
        } else {
            // Empty message
            false
        }
    }

    ///
    /// Creates a signature from a list of arguments
    ///
    pub fn from_expression_arguments<'a>(args: impl IntoIterator<Item=&'a TalkArgument>) -> TalkMessageSignature {
        let arguments = args.into_iter().collect::<SmallVec<[_; 4]>>();

        if arguments.len() == 1 && arguments[0].value.is_none() {
            Self::unary(&arguments[0].name)
        } else {
            Self::with_arguments(arguments.into_iter().map(|arg| &arg.name))
        }
    }

    ///
    /// True if this is a signature for a unary message (one with no arguments)
    ///
    pub fn is_unary(&self) -> bool {
        match self {
            TalkMessageSignature::Unary(_)  => true,
            _                               => false,
        }
    }

    ///
    /// Number of arguments in this message signature
    ///
    pub fn len(&self) -> usize {
        match self {
            TalkMessageSignature::Unary(_)          => 0,
            TalkMessageSignature::Arguments(args)   => args.len(),
        }
    }
}

impl TalkMessageSignatureId {
    ///
    /// Retrieves the signature corresponding to this ID
    ///
    pub fn to_signature(&self) -> TalkMessageSignature {
        SIGNATURE_FOR_ID.lock().unwrap().get(self).unwrap().clone()
    }
}

impl fmt::Debug for TalkMessageSignature {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            TalkMessageSignature::Unary(symbol)     => fmt.write_fmt(format_args!("{:?}", symbol)),
            TalkMessageSignature::Arguments(args)   => fmt.write_fmt(format_args!("{:?}", args.iter().map(|arg| format!("{:?}", arg)).collect::<Vec<_>>().join(" "))),
        }
    }
}

impl fmt::Debug for TalkMessageSignatureId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.to_signature().fmt(fmt)
    }
}
