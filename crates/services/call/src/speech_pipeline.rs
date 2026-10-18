//! Two-stage pipeline that speaks a reply sentence by sentence: while one sentence is
//! sounding, the next one is already being prepared (synthesized). With a slow online voice
//! this removes most of the silence between sentences.

use std::future::Future;

use tokio::sync::mpsc as tokio_mpsc;
use tokio_util::sync::CancellationToken;

use aivt_provider_tts::SpeakRequest;

/// One sentence of a reply with its voice prepared ahead of time: `request` is what to speak
/// (`None` = nothing to say), `audio` the already-synthesized clip (`None` = speak it directly,
/// which is what native voices need).
pub struct PreparedSentence {
    pub text: String,
    pub request: Option<SpeakRequest>,
    pub audio: Option<Vec<u8>>,
}

/// Prepares each incoming sentence with `prepare` and plays them in order with `play`, the two
/// stages overlapping. `on_first_playback` runs right before the first sentence plays. Returns
/// the sentences that started playing (what the user actually heard). Cancellation stops both
/// stages at once.
pub async fn run<Prepare, PrepareFut, Play, PlayFut>(
    mut sentences: tokio_mpsc::UnboundedReceiver<String>,
    cancellation: &CancellationToken,
    prepare: Prepare,
    play: Play,
    on_first_playback: impl Fn(),
) -> Vec<String>
where
    Prepare: Fn(String) -> PrepareFut,
    PrepareFut: Future<Output = PreparedSentence>,
    Play: Fn(PreparedSentence) -> PlayFut,
    PlayFut: Future<Output = ()>,
{
    // Capacity 1: a prepared sentence waits here while the one before it is still sounding.
    let (ready_tx, mut ready_rx) = tokio_mpsc::channel::<PreparedSentence>(1);
    // `async move` so that `ready_tx` is dropped when this stage ends: that is what lets the
    // playing stage see the queue closed and finish.
    let preparing = async move {
        loop {
            let next = tokio::select! {
                biased;
                () = cancellation.cancelled() => None,
                sentence = sentences.recv() => sentence,
            };
            let Some(sentence) = next else { break };
            let prepared = tokio::select! {
                biased;
                () = cancellation.cancelled() => break,
                prepared = prepare(sentence) => prepared,
            };
            tokio::select! {
                biased;
                () = cancellation.cancelled() => break,
                sent = ready_tx.send(prepared) => if sent.is_err() { break },
            }
        }
    };
    let playing = async {
        let mut delivered = Vec::new();
        loop {
            let next = tokio::select! {
                biased;
                () = cancellation.cancelled() => None,
                prepared = ready_rx.recv() => prepared,
            };
            let Some(prepared) = next else { break };
            if delivered.is_empty() {
                on_first_playback();
            }
            delivered.push(prepared.text.clone());
            play(prepared).await;
        }
        delivered
    };
    let ((), delivered) = tokio::join!(preparing, playing);
    delivered
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::*;

    fn sentence(text: String) -> PreparedSentence {
        PreparedSentence { text, request: None, audio: None }
    }

    fn feed(sentences: &[&str]) -> tokio_mpsc::UnboundedReceiver<String> {
        let (tx, rx) = tokio_mpsc::unbounded_channel();
        for s in sentences {
            tx.send((*s).to_string()).unwrap();
        }
        rx
    }

    #[tokio::test]
    async fn the_next_sentence_is_prepared_while_the_current_one_plays() {
        let rx = feed(&["uno", "dos", "tres"]);
        let started = Instant::now();
        let delivered = run(
            rx,
            &CancellationToken::new(),
            |text| async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                sentence(text)
            },
            |_| async { tokio::time::sleep(Duration::from_millis(100)).await },
            || {},
        )
        .await;

        assert_eq!(delivered, vec!["uno", "dos", "tres"]);
        // Sequential would be 3 x (100 + 100) = 600 ms; overlapped it is 100 + 3 x 100 = 400 ms.
        assert!(started.elapsed() < Duration::from_millis(520), "took {:?}", started.elapsed());
    }

    #[tokio::test]
    async fn sentences_play_in_order_and_the_first_playback_hook_runs_once() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let firsts = Arc::new(AtomicUsize::new(0));
        let rx = feed(&["a", "b", "c", "d"]);
        let played = order.clone();
        let counter = firsts.clone();
        run(
            rx,
            &CancellationToken::new(),
            |text| async move { sentence(text) },
            move |prepared| {
                let played = played.clone();
                async move { played.lock().unwrap().push(prepared.text) }
            },
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await;

        assert_eq!(*order.lock().unwrap(), vec!["a", "b", "c", "d"]);
        assert_eq!(firsts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancelling_stops_both_stages_and_reports_only_what_started() {
        let cancellation = CancellationToken::new();
        let rx = feed(&["uno", "dos", "tres"]);
        let stopper = cancellation.clone();
        let delivered = run(
            rx,
            &cancellation,
            |text| async move { sentence(text) },
            move |_| {
                let stopper = stopper.clone();
                async move {
                    // The first sentence is interrupted while it sounds.
                    stopper.cancel();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            },
            || {},
        )
        .await;

        assert_eq!(delivered, vec!["uno"]);
    }

    #[tokio::test]
    async fn no_sentences_means_nothing_delivered() {
        let delivered = run(feed(&[]), &CancellationToken::new(), |t| async move { sentence(t) }, |_| async {}, || panic!("nothing should play")).await;
        assert!(delivered.is_empty());
    }
}
