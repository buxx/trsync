use trsync_core::{
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
};

pub enum StateModification {
    Forgot(ContentId),
    Add(Content),
    Update(
        ContentId,
        ContentFileName,
        RevisionId,
        Option<ContentId>,
        DiskTimestamp,
    ),
    // FIXME BS NOW : use Update ?
    Rename(ContentId, ContentFileName, Option<ContentId>),
}
