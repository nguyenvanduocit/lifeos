//! What an agent is told: lifeos's own system prompt, a crew member's part on top of it, and the lead a new session of
//! an agent opens with, a recap of its conversation so far (remit's `assistant_lead_for`). Pure.

use lifeos_wire::{ActorKind, CrewMember, Event, EventKind};

pub const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

/// The events of the conversation a recap reads, at most, and the characters it keeps of each.
const RECAP_EVENTS: usize = 40;
const RECAP_CHARS: usize = 600;

/// What a crew member is told on top of lifeos's own prompt.
pub fn member_prompt(m: &CrewMember) -> String {
    format!(
        "\n\n# Bạn là {name}\n\nBạn là một thành viên trong crew của người dùng, không phải Assistant chính.{role}\n\n{instructions}\n\n\
         Memory của bạn là của riêng bạn; bạn cũng thấy những gì Assistant nhớ về người dùng.\n\n\
         Bạn nói chuyện trực tiếp với các agent khác: cần điều một agent biết hay chỉ tool của nó chạm tới thì hỏi bằng \
         `ask_agent` (Assistant là `assistant`, thấy toàn bộ cuộc sống của người dùng và có các tool bạn thiếu); phần việc \
         của agent khác thì giao bằng `hand_off`. Kết quả tự quay về thành tin nhắn mở đầu bằng \
         `[what came of what you handed off]`. Tin nhắn mở đầu bằng `[hand-off #N …]` là việc một agent giao bạn: làm \
         bằng tool của bạn; tin nhắn cuối của bạn trong lượt đó là câu trả lời quay về cho nó, nên viết ngắn gọn, đủ ý: \
         đã làm gì, tìm thấy gì, còn gì. Nếu bạn giao tiếp một phần, câu trả lời của bạn chờ phần đó về; khi nó về, kiểm \
         tra, sửa điều nó tìm ra, rồi trả lời.\n\n\
         Khi không có người dùng trong cuộc trò chuyện (một routine, một tin nhắn từ agent khác) mà bạn cần họ quyết hay \
         cần điều chỉ họ biết, hỏi bằng `ask_owner` rồi làm tiếp phần không cần câu trả lời; Assistant quyết thay họ trước \
         và câu trả lời về sau thành tin nhắn. Khi người dùng đang trò chuyện với bạn, cứ hỏi thẳng trong câu trả lời.",
        name = m.name,
        role = m.role.as_deref().map(|r| format!(" Vai trò: {r}.")).unwrap_or_default(),
        instructions = m.instructions.trim(),
    )
}

/// The system prompt of agent `member` (none: the Assistant).
pub fn system_prompt(member: Option<&CrewMember>) -> String {
    let mut prompt = SYSTEM_PROMPT.to_owned();
    if let Some(m) = member {
        prompt.push_str(&member_prompt(m));
    }
    prompt
}

fn cut(text: &str) -> String {
    match text.char_indices().nth(RECAP_CHARS) {
        Some((i, _)) => format!("{}…", text.get(..i).unwrap_or(text)),
        None => text.to_owned(),
    }
}

/// What a new session of an agent reads before its first message: the conversation it had before, newest last, from
/// `stream` (the events before the message, oldest first). None when there was none.
pub fn recap(stream: &[Event]) -> Option<String> {
    let kept: Vec<&Event> = stream
        .iter()
        .filter(|e| match e.kind {
            EventKind::Prompt => e.status == "delivered" || e.status == "steered",
            EventKind::Message => true,
            _ => false,
        })
        .filter(|e| !e.text.trim().is_empty())
        .collect();
    let said: Vec<String> = kept[kept.len().saturating_sub(RECAP_EVENTS)..]
        .iter()
        .map(|e| {
            let who = match (&e.kind, &e.by) {
                (EventKind::Message, _) => "Bạn".to_owned(),
                (_, Some(by)) if by.kind == ActorKind::Owner => "Người dùng".to_owned(),
                (_, Some(by)) => by.name.clone(),
                (_, None) => "lifeos".to_owned(),
            };
            format!("**{who}:** {}", cut(e.text.trim()))
        })
        .collect();
    (!said.is_empty()).then(|| {
        format!(
            "<recap>\nĐây là phiên mới của bạn. Cuộc trò chuyện trước đó, để bạn nối tiếp (đã rút gọn):\n\n{}\n</recap>",
            said.join("\n\n")
        )
    })
}
