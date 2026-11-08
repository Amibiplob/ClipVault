//! Native Wayland clipboard ownership, using standard ext-data-control or wlroots.
//! The service connection stays alive when the floating UI is hidden.
use crate::{
    clipboard::{Command, Notice, Shared},
    storage::Snapshot,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    sync::{
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};
use wayland_client::{
    delegate_noop,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy, QueueHandle,
};

struct Probe {
    ext: bool,
    wlr: bool,
}
impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
    fn event(
        s: &mut Self,
        _: &wl_registry::WlRegistry,
        e: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { interface, .. } = e {
            s.ext |= interface == "ext_data_control_manager_v1";
            s.wlr |= interface == "zwlr_data_control_manager_v1";
        }
    }
}
pub fn run(store: Shared, rx: Receiver<Command>, out: Sender<Notice>) -> Result<(), String> {
    let c = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut q = c.new_event_queue::<Probe>();
    c.display().get_registry(&q.handle(), ());
    let mut p = Probe {
        ext: false,
        wlr: false,
    };
    q.roundtrip(&mut p).map_err(|e| e.to_string())?;
    if p.ext {
        ext::run(store, rx, out)
    } else if p.wlr {
        wlr::run(store, rx, out)
    } else {
        Err("This Wayland desktop does not expose clipboard data-control. GNOME requires a clipboard extension or an X11 session. See Setup in Settings; background capture is unavailable here.".into())
    }
}

macro_rules! driver {($module:ident,$manager:ident,$device:ident,$offer:ident,$source:ident,$Manager:ident,$Device:ident,$Offer:ident,$Source:ident,$interface:literal,$version:literal)=>{
use $module::{$manager::{$Manager as Manager},$device::{self,$Device as Device},$offer::{self,$Offer as Offer},$source::{self,$Source as Source}};
struct State{manager:Option<Manager>,seat:Option<wl_seat::WlSeat>,device:Option<Device>,offers:HashMap<wayland_client::backend::ObjectId,(Offer,Vec<String>)>,pending:Option<Offer>,source:Option<Source>,finished:bool}
impl State{fn bind(&mut self,qh:&QueueHandle<Self>){if self.device.is_none(){if let(Some(m),Some(seat))=(&self.manager,&self.seat){self.device=Some(m.get_data_device(seat,qh,()));}}}}
impl Dispatch<wl_registry::WlRegistry,()> for State{fn event(s:&mut Self,r:&wl_registry::WlRegistry,e:wl_registry::Event,_:&(),_:&Connection,q:&QueueHandle<Self>){if let wl_registry::Event::Global{name,interface,version}=e {if interface==$interface {s.manager=Some(r.bind::<Manager,_,_>(name,version.min($version),q,()));}if interface=="wl_seat"&&s.seat.is_none(){s.seat=Some(r.bind::<wl_seat::WlSeat,_,_>(name,1,q,()));}s.bind(q);}}}
delegate_noop!(State: ignore wl_seat::WlSeat);delegate_noop!(State: ignore Manager);
impl Dispatch<Device,()> for State{
fn event(s:&mut Self,_:&Device,e:$device::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){match e{
 $device::Event::DataOffer{id}=>{s.offers.insert(id.id(),(id,Vec::new()));},
 $device::Event::Selection{id}=>{s.pending=id;},
 $device::Event::Finished=>s.finished=true,_=>{}
}}
wayland_client::event_created_child!(State,Device,[0=>(Offer,())]);
}
impl Dispatch<Offer,()> for State{fn event(s:&mut Self,o:&Offer,e:$offer::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){if let $offer::Event::Offer{mime_type}=e {if let Some((_,v))=s.offers.get_mut(&o.id()){v.push(mime_type);}}}}
impl Dispatch<Source,Arc<Snapshot>> for State{fn event(s:&mut Self,source:&Source,e:$source::Event,data:&Arc<Snapshot>,_:&Connection,_:&QueueHandle<Self>){match e{
 $source::Event::Send{mime_type,fd}=>{if let Some(bytes)=data.get(&mime_type){let bytes=bytes.clone();std::thread::spawn(move||{let mut file=std::fs::File::from(fd);let _=file.write_all(&bytes);});}},
 $source::Event::Cancelled=>{if s.source.as_ref()==Some(source){s.source=None;}source.destroy();},_=>{}
}}}
pub fn run(store:Shared,rx:Receiver<Command>,out:Sender<Notice>)->Result<(),String>{
let c=Connection::connect_to_env().map_err(|e|e.to_string())?;let mut q=c.new_event_queue::<State>();let h=q.handle();c.display().get_registry(&h,());let mut s=State{manager:None,seat:None,device:None,offers:HashMap::new(),pending:None,source:None,finished:false};q.roundtrip(&mut s).map_err(|e|e.to_string())?;q.roundtrip(&mut s).map_err(|e|e.to_string())?;
if s.device.is_none(){return Err("No Wayland clipboard seat is available".into())}
let _=out.send(Notice::Status("Wayland · monitoring clipboard. Bind your desktop shortcut to clipvault --toggle.".into()));let own_mime=format!("application/x-clipvault-owned-{}",std::process::id());
loop{
 while let Ok(cmd)=rx.try_recv(){match cmd{Command::Stop=>return Ok(()),Command::RecordShortcut(_)=>{let _=out.send(Notice::ShortcutFailed("Direct global shortcuts are unavailable on Wayland. Configure your desktop shortcut to run clipvault --toggle; the previous setting is unchanged.".into()));},Command::Restore(id,data)=>{
 if data.is_empty(){let _=out.send(Notice::ActivationFailed(id,"The saved clip has no readable clipboard formats".into()));continue;}
 let data=Arc::new(crate::clipboard::compatible_text(data));
 let src=s.manager.as_ref().unwrap().create_data_source(&h,data.clone());
 for mime in data.keys(){src.offer(mime.clone());}src.offer(own_mime.clone());
 s.device.as_ref().unwrap().set_selection(Some(&src));s.source=Some(src);
 c.flush().map_err(|e|e.to_string())?;q.roundtrip(&mut s).map_err(|e|e.to_string())?;
 if s.finished||s.source.is_none(){let _=out.send(Notice::ActivationFailed(id,"The Wayland compositor rejected clipboard activation. The window will stay open; check clipboard permissions and try again.".into()));continue;}
 if let Err(e)=store.lock().unwrap().touch(&id){let _=out.send(Notice::ActivationFailed(id,format!("Clipboard ready, but recency could not be saved: {e}")));}else{let _=out.send(Notice::Activated(id));}
 }}}
 q.dispatch_pending(&mut s).map_err(|e|e.to_string())?;
 if s.finished{return Err("Wayland clipboard permission was revoked by the compositor".into())}
 if let Some(offer)=s.pending.take(){let settings=store.lock().unwrap().settings.clone();let mimes=s.offers.get(&offer.id()).map(|(_,m)|m.clone()).unwrap_or_default();if !settings.paused && !mimes.contains(&own_mime) && !mimes.iter().any(|m|crate::clipboard::sensitive_mime(m)) {
 let mut data=Snapshot::new();let mut total=0;let mut failed=false;let cap=settings.max_mb*1024*1024;
 for mime in mimes.into_iter().filter(|m|crate::clipboard::supported(m)).take(24){
  let mut fds=[0;2];if unsafe{libc::pipe2(fds.as_mut_ptr(),libc::O_CLOEXEC)}!=0{continue}let read=unsafe{OwnedFd::from_raw_fd(fds[0])};let write=unsafe{OwnedFd::from_raw_fd(fds[1])};unsafe{libc::fcntl(read.as_raw_fd(),libc::F_SETFL,libc::O_NONBLOCK);}
  offer.receive(mime.clone(),write.as_fd());drop(write);c.flush().map_err(|e|e.to_string())?;
  let mut file=std::fs::File::from(read);let mut bytes=Vec::new();let end=Instant::now()+Duration::from_secs(3);let mut complete=false;
  while Instant::now()<end{q.dispatch_pending(&mut s).map_err(|e|e.to_string())?;let mut buf=[0;65536];match file.read(&mut buf){Ok(0)=>{complete=true;break},Ok(n)=>{if total+bytes.len()+n>cap{failed=true;let _=out.send(Notice::Error("Clipboard item exceeds the configured size limit".into()));break}bytes.extend_from_slice(&buf[..n]);},Err(e)if e.kind()==std::io::ErrorKind::WouldBlock=>{if let Some(g)=q.prepare_read(){let mut p=libc::pollfd{fd:g.connection_fd().as_raw_fd(),events:libc::POLLIN,revents:0};if unsafe{libc::poll(&mut p,1,5)}>0{let _=g.read();}}},Err(_)=>break}}
  if complete&&!bytes.is_empty(){total+=bytes.len();data.insert(mime,bytes);}
 }
 if !failed&&data.is_empty(){let _=out.send(Notice::Error("Unsupported clipboard content or incomplete clipboard transfer".into()));}
 if !failed{crate::clipboard::record(&store,data,"Wayland",&out);}
 }
 let old:Vec<_>=s.offers.keys().filter(|id|**id!=offer.id()).cloned().collect();for id in old{if let Some((o,_))=s.offers.remove(&id){o.destroy();}}
 }
 c.flush().map_err(|e|e.to_string())?;if let Some(g)=q.prepare_read(){let mut p=libc::pollfd{fd:g.connection_fd().as_raw_fd(),events:libc::POLLIN,revents:0};if unsafe{libc::poll(&mut p,1,15)}>0{g.read().map_err(|e|e.to_string())?;}}
}
}
};}
mod wlr {
    use super::*;
    use wayland_protocols_wlr::data_control::v1::client as protocol;
    driver!(
        protocol,
        zwlr_data_control_manager_v1,
        zwlr_data_control_device_v1,
        zwlr_data_control_offer_v1,
        zwlr_data_control_source_v1,
        ZwlrDataControlManagerV1,
        ZwlrDataControlDeviceV1,
        ZwlrDataControlOfferV1,
        ZwlrDataControlSourceV1,
        "zwlr_data_control_manager_v1",
        2
    );
}
mod ext {
    use super::*;
    use wayland_protocols::ext::data_control::v1::client as protocol;
    driver!(
        protocol,
        ext_data_control_manager_v1,
        ext_data_control_device_v1,
        ext_data_control_offer_v1,
        ext_data_control_source_v1,
        ExtDataControlManagerV1,
        ExtDataControlDeviceV1,
        ExtDataControlOfferV1,
        ExtDataControlSourceV1,
        "ext_data_control_manager_v1",
        1
    );
}
