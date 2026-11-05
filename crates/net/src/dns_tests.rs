// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use hickory_proto::op::{Message, OpCode, ResponseCode};
use hickory_proto::rr::rdata::{A, MX, SRV};
use hickory_proto::rr::{Name, RData, Record};
use inboxcraft_model::{ErrorKind, Host};

use super::{MAX_RECORDS, Resolver, nameservers, read_mx, read_srv};

fn name(text: &str) -> Name {
    Name::from_ascii(text).unwrap()
}

fn mx(preference: u16, exchange: &str) -> Record {
    Record::from_rdata(name("example.org."), 300, RData::MX(MX::new(preference, name(exchange))))
}

fn srv(priority: u16, weight: u16, port: u16, target: &str) -> Record {
    Record::from_rdata(name("_imaps._tcp.example.org."), 300, RData::SRV(SRV::new(priority, weight, port, name(target))))
}

/// The answer to `query` with `code` and `records`.
fn reply(query: &Message, code: ResponseCode, records: Vec<Record>) -> Message {
    let mut answer = Message::response(query.metadata.id, OpCode::Query);
    answer.metadata.response_code = code;
    for question in &query.queries {
        answer.add_query(question.clone());
    }
    answer.insert_answers(records);
    answer
}

/// What the fake name server sends back for one query, datagram by datagram; nothing is silence.
type Answer = Box<dyn Fn(&Message) -> Vec<Vec<u8>> + Send>;

/// A fake name server on 127.0.0.1 for one UDP query, and for one TCP query when `tcp` answers.
struct Fake {
    addr: SocketAddr,
    threads: Vec<JoinHandle<()>>,
}

impl Fake {
    fn start(udp: Answer, tcp: Option<Answer>) -> Self {
        let (socket, listener) = loop {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            if let Ok(listener) = TcpListener::bind(socket.local_addr().unwrap()) {
                break (socket, listener);
            }
        };
        let addr = socket.local_addr().unwrap();
        let mut threads = vec![thread::spawn(move || {
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buffer = vec![0; 4096];
            let Ok((read, from)) = socket.recv_from(&mut buffer) else { return };
            let query = Message::from_vec(&buffer[..read]).unwrap();
            for datagram in udp(&query) {
                socket.send_to(&datagram, from).unwrap();
            }
        })];
        if let Some(tcp) = tcp {
            threads.push(thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut prefix = [0; 2];
                stream.read_exact(&mut prefix).unwrap();
                let mut query = vec![0; usize::from(u16::from_be_bytes(prefix))];
                stream.read_exact(&mut query).unwrap();
                let query = Message::from_vec(&query).unwrap();
                for answer in tcp(&query) {
                    let length = u16::try_from(answer.len()).unwrap().to_be_bytes();
                    stream.write_all(&[length.as_slice(), &answer].concat()).unwrap();
                }
            }));
        }
        Self { addr, threads }
    }

    fn resolver(&self) -> Resolver {
        Resolver::new(vec![self.addr], Duration::from_secs(2))
    }

    fn finish(self) {
        for thread in self.threads {
            thread.join().unwrap();
        }
    }
}

fn answering(code: ResponseCode, records: Vec<Record>) -> Answer {
    Box::new(move |query| vec![reply(query, code, records.clone()).to_vec().unwrap()])
}

fn hosts(found: &[Host]) -> Vec<&str> {
    found.iter().map(Host::as_str).collect()
}

#[test]
fn the_resolver_configuration_gives_up_to_three_servers() {
    let conf = "# comment\n; other comment\nnameserver 192.0.2.1\nnameserver 192.0.2.1\nnameserver   2001:db8::1 \nnameserver fe80::1%eth0\nnameserver bad\nnameserver\noptions edns0\nnameserver 192.0.2.2\nnameserver 192.0.2.3\n";
    let servers: Vec<String> = nameservers(conf).iter().map(ToString::to_string).collect();
    assert_eq!(servers, ["192.0.2.1:53", "[2001:db8::1]:53", "192.0.2.2:53"]);
    assert!(nameservers("").is_empty());
}

#[test]
fn mx_records_come_by_preference_without_null_or_twice() {
    let query = {
        let mut query = Message::query();
        query.add_query(hickory_proto::op::Query::query(name("example.org."), hickory_proto::rr::RecordType::MX));
        query
    };
    let a = Record::from_rdata(name("example.org."), 300, RData::A(A::new(192, 0, 2, 1)));
    let records = vec![mx(20, "mx2.example.org."), a, mx(10, "mx1.example.org."), mx(30, "."), mx(15, "MX1.example.org.")];
    let bytes = reply(&query, ResponseCode::NoError, records).to_vec().unwrap();
    assert_eq!(hosts(&read_mx(&bytes).unwrap()), ["mx1.example.org", "mx2.example.org"]);
    assert_eq!(read_mx(b"not dns"), None);
    let many: Vec<Record> = (0..40).map(|n| mx(n, &format!("mx{n}.example.org."))).collect();
    let bytes = reply(&query, ResponseCode::NoError, many).to_vec().unwrap();
    assert_eq!(read_mx(&bytes).unwrap().len(), MAX_RECORDS);
}

#[test]
fn srv_records_come_by_priority_then_weight_without_the_empty_target() {
    let query = Message::query();
    let records = vec![
        srv(10, 5, 993, "b.example.org."),
        srv(0, 0, 993, "."),
        srv(10, 50, 993, "a.example.org."),
        srv(5, 0, 0, "zero.example.org."),
        srv(1, 0, 993, "first.example.org."),
    ];
    let found = read_srv(&reply(&query, ResponseCode::NoError, records).to_vec().unwrap()).unwrap();
    let targets: Vec<&str> = found.iter().map(|srv| srv.host.as_str()).collect();
    assert_eq!(targets, ["first.example.org", "a.example.org", "b.example.org"]);
    assert_eq!(found[0].port.get(), 993);
    assert_eq!(read_srv(&[0; 3]), None);
}

#[test]
fn an_mx_query_is_answered_over_udp() {
    let fake = Fake::start(answering(ResponseCode::NoError, vec![mx(10, "mx.example.org.")]), None);
    let found = fake.resolver().mx(&Host::new("Example.org").unwrap()).unwrap();
    assert_eq!(hosts(&found), ["mx.example.org"]);
    fake.finish();
}

#[test]
fn a_name_that_does_not_exist_has_no_records() {
    let fake = Fake::start(answering(ResponseCode::NXDomain, Vec::new()), None);
    assert!(fake.resolver().srv("_imaps._tcp.example.org").unwrap().is_empty());
    fake.finish();
}

#[test]
fn a_failing_server_is_an_error_and_the_next_one_is_asked() {
    let failing = Fake::start(answering(ResponseCode::ServFail, Vec::new()), None);
    assert_eq!(failing.resolver().mx(&Host::new("example.org").unwrap()).unwrap_err().kind(), ErrorKind::CommandRefused);
    failing.finish();

    let failing = Fake::start(answering(ResponseCode::Refused, Vec::new()), None);
    let working = Fake::start(answering(ResponseCode::NoError, vec![mx(10, "mx.example.org.")]), None);
    let resolver = Resolver::new(vec![failing.addr, working.addr], Duration::from_secs(2));
    assert_eq!(hosts(&resolver.mx(&Host::new("example.org").unwrap()).unwrap()), ["mx.example.org"]);
    failing.finish();
    working.finish();
}

#[test]
fn stray_datagrams_are_ignored() {
    let strays: Answer = Box::new(|query: &Message| {
        let mut wrong_id = reply(query, ResponseCode::NoError, vec![mx(1, "evil.example.net.")]);
        wrong_id.metadata.id = query.metadata.id.wrapping_add(1);
        let mut wrong_name = reply(query, ResponseCode::NoError, vec![mx(1, "evil.example.net.")]);
        wrong_name.queries[0] = hickory_proto::op::Query::query(name("example.net."), query.queries[0].query_type());
        let mut not_a_response = reply(query, ResponseCode::NoError, vec![mx(1, "evil.example.net.")]);
        not_a_response.metadata.message_type = hickory_proto::op::MessageType::Query;
        let mut no_question = reply(query, ResponseCode::NoError, vec![]);
        no_question.queries.clear();
        let right = reply(query, ResponseCode::NoError, vec![mx(10, "mx.example.org.")]);
        [wrong_id, wrong_name, not_a_response, no_question]
            .iter()
            .map(|message| message.to_vec().unwrap())
            .chain([b"junk".to_vec(), right.to_vec().unwrap()])
            .collect()
    });
    let fake = Fake::start(strays, None);
    assert_eq!(hosts(&fake.resolver().mx(&Host::new("example.org").unwrap()).unwrap()), ["mx.example.org"]);
    fake.finish();
}

#[test]
fn too_many_strays_give_the_server_up() {
    let fake = Fake::start(Box::new(|_| vec![b"junk".to_vec(); 10]), None);
    assert_eq!(fake.resolver().mx(&Host::new("example.org").unwrap()).unwrap_err().kind(), ErrorKind::Protocol);
    fake.finish();
}

#[test]
fn a_silent_server_times_out() {
    let fake = Fake::start(Box::new(|_| Vec::new()), None);
    let resolver = Resolver::new(vec![fake.addr], Duration::from_millis(200));
    assert_eq!(resolver.mx(&Host::new("example.org").unwrap()).unwrap_err().kind(), ErrorKind::TimedOut);
    fake.finish();
}

#[test]
fn a_truncated_answer_is_asked_again_over_tcp() {
    let truncated: Answer = Box::new(|query: &Message| {
        let mut answer = reply(query, ResponseCode::NoError, Vec::new());
        answer.metadata.truncation = true;
        vec![answer.to_vec().unwrap()]
    });
    let fake = Fake::start(truncated, Some(answering(ResponseCode::NoError, vec![mx(10, "big.example.org.")])));
    assert_eq!(hosts(&fake.resolver().mx(&Host::new("example.org").unwrap()).unwrap()), ["big.example.org"]);
    fake.finish();

    let truncated: Answer = Box::new(|query: &Message| {
        let mut answer = reply(query, ResponseCode::NoError, Vec::new());
        answer.metadata.truncation = true;
        vec![answer.to_vec().unwrap()]
    });
    let fake = Fake::start(truncated, Some(Box::new(|_| vec![b"junk".to_vec()])));
    assert_eq!(fake.resolver().mx(&Host::new("example.org").unwrap()).unwrap_err().kind(), ErrorKind::Protocol);
    fake.finish();
}

#[test]
fn without_a_server_or_with_a_bad_name_nothing_is_asked() {
    let resolver = Resolver::new(Vec::new(), Duration::from_secs(1));
    assert_eq!(resolver.mx(&Host::new("example.org").unwrap()).unwrap_err().kind(), ErrorKind::HostNotFound);
    assert_eq!(resolver.srv(&"a".repeat(300)).unwrap_err().kind(), ErrorKind::HostNotFound);
    assert!(format!("{resolver:?}").contains("Resolver"));
}

#[test]
fn the_system_configuration_is_read_where_there_is_one() {
    // The container may or may not have one; either way the call answers.
    match Resolver::system(Duration::from_secs(1)) {
        Ok(resolver) => assert!(format!("{resolver:?}").contains(":53")),
        Err(error) => assert_eq!(error.kind(), ErrorKind::HostNotFound),
    }
}
