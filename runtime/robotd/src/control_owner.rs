//! Single authority for the two live control inputs. No torque or bus access.
use serde::Serialize;

const FRESH_US: u64 = 1_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source { Pad, Web }
impl Source {
    pub fn parse(s: &str) -> Option<Self> { match s { "pad"=>Some(Self::Pad), "web"=>Some(Self::Web), _=>None } }
    fn slot(self)->usize { if self==Self::Pad {0} else {1} }
}
#[derive(Clone, Copy, Default)]
struct Input { token:u64, last:u64, online:bool, neutral:bool }
#[derive(Serialize)]
pub struct Status {
    pub owner:Option<Source>, pub preference:&'static str,
    pub pad_connected:bool, pub web_connected:bool, pub ready:bool,
    pub generation:u64, pub reason:&'static str,
}
pub struct Manager {
    inputs:[Input;2], owner:Option<Source>, token:u64,
    manual_web:bool, ready:bool, generation:u64, changed:bool, reason:&'static str,
}
impl Default for Manager {
    fn default()->Self { Self {inputs:[Input::default();2],owner:None,token:0,manual_web:false,
        ready:false,generation:0,changed:false,reason:"boot_waiting_for_pad"} }
}
impl Manager {
    pub fn tick(&mut self, now:u64) {
        for input in &mut self.inputs { if now.saturating_sub(input.last)>FRESH_US {input.online=false;} }
        if !self.inputs[1].online {self.manual_web=false;}
        let owner=if self.manual_web {Some(Source::Web)} else if self.inputs[0].online {Some(Source::Pad)}
            else if self.inputs[1].online {Some(Source::Web)} else {None};
        let token=owner.map(|s|self.inputs[s.slot()].token).unwrap_or(0);
        if owner!=self.owner || token!=self.token {
            self.owner=owner;self.token=token;self.ready=false;self.generation+=1;self.changed=true;
            self.reason=match owner {Some(Source::Pad)=>"pad_priority",Some(Source::Web) if self.manual_web=>"manual_web",
                Some(Source::Web)=>"pad_unavailable_web",None=>"no_controller"};
        }
    }
    pub fn heartbeat(&mut self, source:Source, token:u64, online:bool, neutral:bool, now:u64) {
        self.tick(now);
        let generation=self.generation;
        self.inputs[source.slot()]=Input {token,last:now,online,neutral};
        self.tick(now);
        if self.generation==generation && self.owner==Some(source) && online && neutral {self.ready=true;}
    }
    pub fn disconnect(&mut self, source:Source,token:u64,now:u64) {
        if self.inputs[source.slot()].token==token {self.inputs[source.slot()].online=false;}
        self.tick(now);
    }
    pub fn select(&mut self, preference:&str, now:u64)->Result<(), &'static str> {
        self.tick(now);
        match preference {
            "web" if self.inputs[1].online=>self.manual_web=true,
            "web"=>return Err("web controller is not connected"),
            "auto"=> {
                if self.inputs[0].online && !self.inputs[0].neutral {return Err("release gamepad sticks, triggers and buttons first");}
                self.manual_web=false;
            },
            _=>return Err("preference must be web or auto"),
        }
        self.tick(now);Ok(())
    }
    pub fn allows(&self,source:Source,token:u64)->bool {self.owner==Some(source)&&self.token==token&&self.ready}
    pub fn take_changed(&mut self)->bool {std::mem::take(&mut self.changed)}
    pub fn status(&self)->Status {Status {owner:self.owner,preference:if self.manual_web {"web"} else {"auto"},
        pad_connected:self.inputs[0].online,web_connected:self.inputs[1].online,ready:self.ready,
        generation:self.generation,reason:self.reason}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pad_priority_passive_web_cannot_steal() {
        let mut m=Manager::default();m.heartbeat(Source::Pad,1,true,true,1);m.heartbeat(Source::Pad,1,true,true,2);
        m.heartbeat(Source::Web,2,true,true,3);assert!(m.allows(Source::Pad,1));assert!(!m.allows(Source::Web,2));
    }
    #[test] fn fallback_and_reconnect_require_neutral_and_new_action() {
        let mut m=Manager::default();m.heartbeat(Source::Web,2,true,true,1);m.heartbeat(Source::Web,2,true,true,2);
        assert!(m.allows(Source::Web,2));m.heartbeat(Source::Pad,1,true,false,3);
        assert!(!m.allows(Source::Web,2));assert!(!m.allows(Source::Pad,1));
        m.heartbeat(Source::Pad,1,true,false,4);assert!(!m.allows(Source::Pad,1));
        m.heartbeat(Source::Pad,1,true,true,5);assert!(m.allows(Source::Pad,1));
        m.disconnect(Source::Pad,1,6);assert_eq!(m.status().owner,Some(Source::Web));assert!(!m.status().ready);
    }
    #[test] fn manual_web_persists_despite_pad_reconnect_until_auto_or_loss() {
        let mut m=Manager::default();m.heartbeat(Source::Pad,1,true,true,1);m.heartbeat(Source::Web,2,true,true,2);
        m.select("web",3).unwrap();m.heartbeat(Source::Web,2,true,true,4);m.heartbeat(Source::Pad,3,true,true,5);
        assert!(m.allows(Source::Web,2));m.select("auto",6).unwrap();assert_eq!(m.status().owner,Some(Source::Pad));
        assert!(!m.status().ready);m.select("web",7).unwrap();m.disconnect(Source::Web,2,8);
        assert_eq!(m.status().owner,Some(Source::Pad));assert_eq!(m.status().preference,"auto");
    }
    #[test] fn timeout_and_old_connection_do_not_mask_disconnect() {
        let mut m=Manager::default();m.heartbeat(Source::Pad,1,true,true,1);m.heartbeat(Source::Pad,2,true,true,2);
        m.disconnect(Source::Pad,1,3);assert!(m.status().pad_connected);m.tick(1_000_003);
        assert_eq!(m.status().owner,None);assert!(!m.status().ready);
    }
    #[test] fn nonneutral_manual_return_is_refused() {
        let mut m=Manager::default();m.heartbeat(Source::Web,2,true,true,1);m.select("web",2).unwrap();
        m.heartbeat(Source::Pad,1,true,false,3);assert!(m.select("auto",4).is_err());assert_eq!(m.status().owner,Some(Source::Web));
    }
}
