//! Reply codes of the `AniDB` UDP API.
//!
//! The table below has the 143 codes of the spec's Return Codes table, 604 from
//! the Basics section (it is not in that table), and the client side codes 670
//! to 673.
//!
//! The spec lists the reply codes in its section Return Codes. A reply is a
//! three digit code, followed by a text. The spec asks clients to work with the
//! numbers, the names are the strings the server sends in its text.
//!
//! Codes 670 to 673 are client side codes: the server never sends them, and
//! they are not in the spec. They describe failures of the client itself, an
//! unknown error, a timeout, no connection, or a failed initialisation.
//!
//! Codes that any command may return, according to the section Basics: 505,
//! 555, 598, 600, 601, 602 and 604, and for commands that need a login 501, 502
//! and 506. Codes above 305 that a command does not expect are errors, see
//! [`crate::error::Error::from_reply`]. Codes up to 305 are answers.
//!
//! The codes 504 and 555 (bans), and the codes 6xx (server errors), are sent
//! without a tag. See [`crate::wire`] for how replies are matched.

use std::fmt;

macro_rules! response_codes {
    ($( $variant:ident = $code:literal => $label:literal, )*) => {
        /// A reply code of the `AniDB` UDP API.
        ///
        /// Codes the table does not list are kept as `Unknown`, so a newer server
        /// never turns into a parse failure.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum ResponseCode {
            $( #[doc = concat!("`", $label, "`: ", stringify!($code))] $variant, )*
            /// A code that is not in the table.
            Unknown(u16),
        }

        impl ResponseCode {
            /// The numeric code as the server sends it.
            #[must_use]
            pub const fn code(self) -> u16 {
                match self {
                    $( Self::$variant => $code, )*
                    Self::Unknown(code) => code,
                }
            }

            /// The name of the code, `None` for codes the table does not list.
            #[must_use]
            pub const fn label(self) -> Option<&'static str> {
                match self {
                    $( Self::$variant => Some($label), )*
                    Self::Unknown(_) => None,
                }
            }
        }

        impl From<u16> for ResponseCode {
            fn from(code: u16) -> Self {
                match code {
                    $( $code => Self::$variant, )*
                    other => Self::Unknown(other),
                }
            }
        }
    };
}

response_codes! {
    LoginAccepted = 200 => "LOGIN_ACCEPTED",
    LoginAcceptedNewVersion = 201 => "LOGIN_ACCEPTED_NEW_VERSION",
    LoggedOut = 203 => "LOGGED_OUT",
    Resource = 205 => "RESOURCE",
    Stats = 206 => "STATS",
    Top = 207 => "TOP",
    Uptime = 208 => "UPTIME",
    EncryptionEnabled = 209 => "ENCRYPTION_ENABLED",
    MylistEntryAdded = 210 => "MYLIST_ENTRY_ADDED",
    MylistEntryDeleted = 211 => "MYLIST_ENTRY_DELETED",
    AddedFile = 214 => "ADDED_FILE",
    AddedStream = 215 => "ADDED_STREAM",
    ExportQueued = 217 => "EXPORT_QUEUED",
    ExportCancelled = 218 => "EXPORT_CANCELLED",
    EncodingChanged = 219 => "ENCODING_CHANGED",
    File = 220 => "FILE",
    Mylist = 221 => "MYLIST",
    MylistStats = 222 => "MYLIST_STATS",
    Wishlist = 223 => "WISHLIST",
    Notification = 224 => "NOTIFICATION",
    GroupStatus = 225 => "GROUP_STATUS",
    WishlistEntryAdded = 226 => "WISHLIST_ENTRY_ADDED",
    WishlistEntryDeleted = 227 => "WISHLIST_ENTRY_DELETED",
    WishlistEntryUpdated = 228 => "WISHLIST_ENTRY_UPDATED",
    MultipleWishlist = 229 => "MULTIPLE_WISHLIST",
    Anime = 230 => "ANIME",
    AnimeBestMatch = 231 => "ANIME_BEST_MATCH",
    RandomAnime = 232 => "RANDOM_ANIME",
    AnimeDescription = 233 => "ANIME_DESCRIPTION",
    Review = 234 => "REVIEW",
    Character = 235 => "CHARACTER",
    Song = 236 => "SONG",
    Animetag = 237 => "ANIMETAG",
    Charactertag = 238 => "CHARACTERTAG",
    Episode = 240 => "EPISODE",
    Updated = 243 => "UPDATED",
    Title = 244 => "TITLE",
    Creator = 245 => "CREATOR",
    NotificationEntryAdded = 246 => "NOTIFICATION_ENTRY_ADDED",
    NotificationEntryDeleted = 247 => "NOTIFICATION_ENTRY_DELETED",
    NotificationEntryUpdate = 248 => "NOTIFICATION_ENTRY_UPDATE",
    MultipleNotification = 249 => "MULTIPLE_NOTIFICATION",
    Group = 250 => "GROUP",
    Category = 251 => "CATEGORY",
    BuddyList = 253 => "BUDDY_LIST",
    BuddyState = 254 => "BUDDY_STATE",
    BuddyAdded = 255 => "BUDDY_ADDED",
    BuddyDeleted = 256 => "BUDDY_DELETED",
    BuddyAccepted = 257 => "BUDDY_ACCEPTED",
    BuddyDenied = 258 => "BUDDY_DENIED",
    Voted = 260 => "VOTED",
    VoteFound = 261 => "VOTE_FOUND",
    VoteUpdated = 262 => "VOTE_UPDATED",
    VoteRevoked = 263 => "VOTE_REVOKED",
    HotAnime = 265 => "HOT_ANIME",
    RandomRecommendation = 266 => "RANDOM_RECOMMENDATION",
    RandomSimilar = 267 => "RANDOM_SIMILAR",
    NotificationEnabled = 270 => "NOTIFICATION_ENABLED",
    NotifyackSuccessfulMessage = 281 => "NOTIFYACK_SUCCESSFUL_MESSAGE",
    NotifyackSuccessfulNotification = 282 => "NOTIFYACK_SUCCESSFUL_NOTIFICATION",
    NotificationState = 290 => "NOTIFICATION_STATE",
    Notifylist = 291 => "NOTIFYLIST",
    NotifygetMessage = 292 => "NOTIFYGET_MESSAGE",
    NotifygetNotify = 293 => "NOTIFYGET_NOTIFY",
    SendmessageSuccessful = 294 => "SENDMESSAGE_SUCCESSFUL",
    UserId = 295 => "USER_ID",
    Calendar = 297 => "CALENDAR",
    Pong = 300 => "PONG",
    Authpong = 301 => "AUTHPONG",
    NoSuchResource = 305 => "NO_SUCH_RESOURCE",
    ApiPasswordNotDefined = 309 => "API_PASSWORD_NOT_DEFINED",
    FileAlreadyInMylist = 310 => "FILE_ALREADY_IN_MYLIST",
    MylistEntryEdited = 311 => "MYLIST_ENTRY_EDITED",
    MultipleMylistEntries = 312 => "MULTIPLE_MYLIST_ENTRIES",
    Watched = 313 => "WATCHED",
    SizeHashExists = 314 => "SIZE_HASH_EXISTS",
    InvalidData = 315 => "INVALID_DATA",
    StreamnoidUsed = 316 => "STREAMNOID_USED",
    ExportNoSuchTemplate = 317 => "EXPORT_NO_SUCH_TEMPLATE",
    ExportAlreadyInQueue = 318 => "EXPORT_ALREADY_IN_QUEUE",
    ExportNoExportQueuedOrIsProcessing = 319 => "EXPORT_NO_EXPORT_QUEUED_OR_IS_PROCESSING",
    NoSuchFile = 320 => "NO_SUCH_FILE",
    NoSuchEntry = 321 => "NO_SUCH_ENTRY",
    MultipleFilesFound = 322 => "MULTIPLE_FILES_FOUND",
    NoSuchWishlist = 323 => "NO_SUCH_WISHLIST",
    NoSuchNotification = 324 => "NO_SUCH_NOTIFICATION",
    NoGroupsFound = 325 => "NO_GROUPS_FOUND",
    NoSuchAnime = 330 => "NO_SUCH_ANIME",
    NoSuchDescription = 333 => "NO_SUCH_DESCRIPTION",
    NoSuchReview = 334 => "NO_SUCH_REVIEW",
    NoSuchCharacter = 335 => "NO_SUCH_CHARACTER",
    NoSuchSong = 336 => "NO_SUCH_SONG",
    NoSuchAnimetag = 337 => "NO_SUCH_ANIMETAG",
    NoSuchCharactertag = 338 => "NO_SUCH_CHARACTERTAG",
    NoSuchEpisode = 340 => "NO_SUCH_EPISODE",
    NoSuchUpdates = 343 => "NO_SUCH_UPDATES",
    NoSuchTitles = 344 => "NO_SUCH_TITLES",
    NoSuchCreator = 345 => "NO_SUCH_CREATOR",
    NoSuchGroup = 350 => "NO_SUCH_GROUP",
    NoSuchCategory = 351 => "NO_SUCH_CATEGORY",
    BuddyAlreadyAdded = 355 => "BUDDY_ALREADY_ADDED",
    NoSuchBuddy = 356 => "NO_SUCH_BUDDY",
    BuddyAlreadyAccepted = 357 => "BUDDY_ALREADY_ACCEPTED",
    BuddyAlreadyDenied = 358 => "BUDDY_ALREADY_DENIED",
    NoSuchVote = 360 => "NO_SUCH_VOTE",
    InvalidVoteType = 361 => "INVALID_VOTE_TYPE",
    InvalidVoteValue = 362 => "INVALID_VOTE_VALUE",
    PermvoteNotAllowed = 363 => "PERMVOTE_NOT_ALLOWED",
    AlreadyPermvoted = 364 => "ALREADY_PERMVOTED",
    HotAnimeEmpty = 365 => "HOT_ANIME_EMPTY",
    RandomRecommendationEmpty = 366 => "RANDOM_RECOMMENDATION_EMPTY",
    RandomSimilarEmpty = 367 => "RANDOM_SIMILAR_EMPTY",
    NotificationDisabled = 370 => "NOTIFICATION_DISABLED",
    NoSuchEntryMessage = 381 => "NO_SUCH_ENTRY_MESSAGE",
    NoSuchEntryNotification = 382 => "NO_SUCH_ENTRY_NOTIFICATION",
    NoSuchMessage = 392 => "NO_SUCH_MESSAGE",
    NoSuchNotify = 393 => "NO_SUCH_NOTIFY",
    NoSuchUser = 394 => "NO_SUCH_USER",
    CalendarEmpty = 397 => "CALENDAR_EMPTY",
    NoChanges = 399 => "NO_CHANGES",
    NotLoggedIn = 403 => "NOT_LOGGED_IN",
    NoSuchMylistFile = 410 => "NO_SUCH_MYLIST_FILE",
    NoSuchMylistEntry = 411 => "NO_SUCH_MYLIST_ENTRY",
    MylistUnavailable = 412 => "MYLIST_UNAVAILABLE",
    LoginFailed = 500 => "LOGIN_FAILED",
    LoginFirst = 501 => "LOGIN_FIRST",
    AccessDenied = 502 => "ACCESS_DENIED",
    ClientVersionOutdated = 503 => "CLIENT_VERSION_OUTDATED",
    ClientBanned = 504 => "CLIENT_BANNED",
    IllegalInputOrAccessDenied = 505 => "ILLEGAL_INPUT_OR_ACCESS_DENIED",
    InvalidSession = 506 => "INVALID_SESSION",
    NoSuchEncryptionType = 509 => "NO_SUCH_ENCRYPTION_TYPE",
    EncodingNotSupported = 519 => "ENCODING_NOT_SUPPORTED",
    Banned = 555 => "BANNED",
    UnknownCommand = 598 => "UNKNOWN_COMMAND",
    InternalServerError = 600 => "INTERNAL_SERVER_ERROR",
    AnidbOutOfService = 601 => "ANIDB_OUT_OF_SERVICE",
    ServerBusy = 602 => "SERVER_BUSY",
    NoData = 603 => "NO_DATA",
    TimeoutDelayAndResubmit = 604 => "TIMEOUT_DELAY_AND_RESUBMIT",
    ApiViolation = 666 => "API_VIOLATION",
    PushackConfirmed = 701 => "PUSHACK_CONFIRMED",
    NoSuchPacketPending = 702 => "NO_SUCH_PACKET_PENDING",
    Version = 998 => "VERSION",
    UnknownApiError = 670 => "UNKNOWN_API_ERROR",
    ConnectionTimeout = 671 => "CONNECTION_TIMEOUT",
    NotConnected = 672 => "NOT_CONNECTED",
    FailedToInitialize = 673 => "FAILED_TO_INITIALIZE",
}

impl ResponseCode {
    /// Whether the spec says to wait before trying again: 504 and 555 (bans)
    /// and 601 (out of service).
    #[must_use]
    pub const fn asks_to_wait(self) -> bool {
        matches!(
            self,
            Self::Banned | Self::ClientBanned | Self::AnidbOutOfService
        )
    }
}

impl From<ResponseCode> for u16 {
    fn from(code: ResponseCode) -> Self {
        code.code()
    }
}

impl fmt::Display for ResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code(), self.label().unwrap_or("UNKNOWN"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes_map_both_ways() {
        assert_eq!(ResponseCode::from(200), ResponseCode::LoginAccepted);
        assert_eq!(ResponseCode::from(320), ResponseCode::NoSuchFile);
        assert_eq!(ResponseCode::from(555), ResponseCode::Banned);
        assert_eq!(ResponseCode::from(670), ResponseCode::UnknownApiError);
        assert_eq!(ResponseCode::NoSuchFile.code(), 320);
        assert_eq!(ResponseCode::NoSuchFile.label(), Some("NO_SUCH_FILE"));
        assert_eq!(u16::from(ResponseCode::Banned), 555);
    }

    #[test]
    fn bans_and_outages_ask_to_wait() {
        assert!(ResponseCode::Banned.asks_to_wait());
        assert!(ResponseCode::ClientBanned.asks_to_wait());
        assert!(ResponseCode::AnidbOutOfService.asks_to_wait());
        assert!(!ResponseCode::NoSuchFile.asks_to_wait());
        assert!(!ResponseCode::ConnectionTimeout.asks_to_wait());
    }

    #[test]
    fn unlisted_codes_are_kept() {
        assert_eq!(ResponseCode::from(999), ResponseCode::Unknown(999));
        assert_eq!(ResponseCode::Unknown(999).code(), 999);
        assert_eq!(ResponseCode::Unknown(999).label(), None);
        assert_eq!(ResponseCode::Unknown(999).to_string(), "999 UNKNOWN");
        assert_eq!(ResponseCode::NoSuchFile.to_string(), "320 NO_SUCH_FILE");
    }
}
