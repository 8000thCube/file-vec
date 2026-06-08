impl<F:FnOnce()> Drop for FinalizeDrop<F>{
	fn drop(&mut self){
		unsafe{self.0.assume_init_read()()}
	}
}
impl<F:FnOnce()> FinalizeDrop<F>{
	/// create a new drop finalizer from the function
	pub fn new(inner:F)->Self{Self(MaybeUninit::new(inner))}
}

#[cfg(test)]
mod tests{
	#[cfg(feature="serial-json")]
	mod json_tests{
		#[test]
		fn close_save_load(){
			let names=["Kyon","Kyouma","Dita","Umika","Roger","Kobato","Kate","Lucy","Lain","Sasami"];

			let mut data:FileVec<Record>=(0..10).map(|n|{
				let id=n as u64+1;
				let measure=rand::random();
				let name=names[n].to_string();

				Record{id,measure,name}
			}).collect();
			let datamem:Vec<Record>=data.to_vec();
			let datapath=data.path().unwrap().to_path_buf();

			data.serialize_on_close(crate::vec::SerialJson::new(false));

			mem::drop(data);

			let mut datanew:FileVec<Record>=FileVec::open_serial(datapath,crate::vec::SerialJson::new(false)).unwrap();
			datanew.set_persistent(false);

			assert_eq!(datamem.as_slice(),datanew.as_slice());
		}
		#[test]
		fn use_another_path(){
			let names=["Kyon","Kyouma","Dita","Umika","Roger","Kobato","Kate","Lucy","Lain","Sasami"];

			let mut data:FileVec<Record>=(0..10).map(|n|{
				let id=n as u64+1;
				let measure=rand::random();
				let name=names[n].to_string();

				Record{id,measure,name}
			}).collect();
			let datamem:Vec<Record>=data.to_vec();

			data.serialize_on_close(crate::vec::SerialJson::new(false));
			data.set_close_behavior(OnClose::serialize_to("another path"));

			mem::drop(data);

			let mut datanew:FileVec<Record>=FileVec::open_serial("another path",crate::vec::SerialJson::new(false)).unwrap();
			datanew.set_persistent(false);

			assert_eq!(datamem.as_slice(),datanew.as_slice());
		}

		#[derive(Clone,Debug,Deserialize,PartialEq,Serialize)]
		pub struct Record{pub id:u64,pub measure:f32,pub name:String}

		use crate::vec::OnClose;
		use serde::{Deserialize,Serialize};
		use super::*;
	}
	#[cfg(feature="serial-rmp")]
	mod rmp_tests{
		#[test]
		fn close_save_load(){
			let names=["Kyon","Kyouma","Dita","Umika","Roger","Kobato","Kate","Lucy","Lain","Sasami"];

			let mut data:FileVec<Record>=(0..10).map(|n|{
				let id=n as u64+1;
				let measure=rand::random();
				let name=names[n].to_string();

				Record{id,measure,name}
			}).collect();
			let datamem:Vec<Record>=data.to_vec();
			let datapath=data.path().unwrap().to_path_buf();

			data.enable_serialization();
			data.set_close_behavior(OnClose::Serialize(None));

			mem::drop(data);

			let mut datanew:FileVec<Record>=FileVec::open_serial(datapath,crate::vec::SerialJson::new(false)).unwrap();
			datanew.set_persistent(false);

			assert_eq!(datamem.as_slice(),datanew.as_slice());
		}

		#[derive(Clone,Debug,Deserialize,PartialEq,Serialize)]
		pub struct Record{pub id:u64,pub measure:f32,pub name:String}

		use crate::vec::OnClose;
		use serde::{Deserialize,Serialize};
		use super::*;
	}

	/*#[test]
	fn hhh(){
		use std::fs::File;
		use std::io::Write;


		let mut v = FileVec::new();
		let a = v.insert_mut(0, String::from("hello world"));

		for n in 0..100{
			v.push(String::new());
		}

		// get the right file name tho
		let mut file = File::open(v.path().unwrap()).unwrap();
		file.write(b"garbage").unwrap();

		if v.len()==1{
			println!("{}", v[0]);
		}
	}*/
	#[test]
	fn close_drop(){
		let data=[Arc::new(1),Arc::new(2),Arc::new(3),Arc::new(4),Arc::new(5)];
		let mut v=FileVec::new();

		for x in data.iter().cloned(){v.push(x)}
		let path=PathBuf::from(v.path().unwrap());

		assert_eq!(Arc::strong_count(&data[0]),2);
		assert_eq!(Arc::strong_count(&data[1]),2);
		assert_eq!(Arc::strong_count(&data[2]),2);
		assert_eq!(Arc::strong_count(&data[3]),2);
		assert_eq!(Arc::strong_count(&data[4]),2);

		v.close();
		v.push(data[0].clone());

		assert_eq!(Arc::strong_count(&data[0]),2);
		assert_eq!(Arc::strong_count(&data[1]),1);
		assert_eq!(Arc::strong_count(&data[2]),1);
		assert_eq!(Arc::strong_count(&data[3]),1);
		assert_eq!(Arc::strong_count(&data[4]),1);
		assert_ne!(path,v.path().unwrap());

		mem::drop(v);

		assert_eq!(Arc::strong_count(&data[0]),1);
	}
	#[test]
	fn dedup_by(){
		let mut v=FileVec::new();

		v.push(5);
		v.push(4);
		v.push(3);
		v.push(3);
		v.push(2);
		v.push(1);
		v.push(0);

		v.dedup_by(|x,y|*x/2==*y/2);
		assert_eq!([5,3,1],v.as_slice());
	}
	#[test]
	fn drain_keep(){
		let mut v=FileVec::new();

		v.extend_from_slice(&[10,9,8,7,6,5,4,3,2,1]);

		let mut drain=v.drain(1..9);

		assert_eq!(drain.next(),Some(9));
		assert_eq!(drain.next(),Some(8));
		assert_eq!(drain.next_back(),Some(2));

		assert_eq!(drain.as_slice(),[7,6,5,4,3]);
		drain.keep_rest();

		assert_eq!(drain.next(),None);
		mem::drop(drain);

		assert_eq!(v.as_slice(),[10,7,6,5,4,3,1]);

		drain=v.drain(..);
		drain.keep_rest();
		mem::drop(drain);

		assert_eq!(v.as_slice(),[10,7,6,5,4,3,1]);
	}
	#[test]
	fn drain_remove(){
		let mut v=FileVec::new();

		v.push(5);
		v.push(4);
		v.push(3);
		v.push(2);
		v.push(1);

		assert_eq!(v.drain(..3).as_slice(),[5,4,3]);
		assert_eq!([2,1],v.as_slice());

		v.push(2);
		v.push(3);
		v.push(4);

		assert_eq!(v.drain(2..4).as_slice(),[2,3]);
		assert_eq!([2,1,4],v.as_slice());

		assert_eq!(v.drain(1..2).as_slice(),[1]);
		assert_eq!([2,4],v.as_slice());

		assert_eq!(v.drain(1..1).as_slice(),[0_i32;0]);
		assert_eq!([2,4],v.as_slice());

		v.push(6);
		v.push(8);
		v.push(10);
		v.push(11);
		v.push(12);

		assert_eq!(v.drain(4..).as_slice(),[10,11,12]);
		assert_eq!([2,4,6,8],v.as_slice());

		assert_eq!(v.drain(..).as_slice(),[2,4,6,8]);
		assert_eq!([0_i32;0],v.as_slice());
	}
	#[test]
	fn extend_iter(){
		let mut v=FileVec::new();

		v.extend([5,4,3,2,1].iter().copied());
		assert_eq!([5,4,3,2,1],v.as_slice());

		v.extend([0,3,9,6].iter().copied());
		assert_eq!([5,4,3,2,1,0,3,9,6],v.as_slice());
	}
	#[test]
	fn extend_slice(){
		let mut v=FileVec::new();

		v.extend_from_slice(&[5,4,3,2,1]);
		assert_eq!([5,4,3,2,1],v.as_slice());

		v.extend_from_slice(&[0,3,9,6]);
		assert_eq!([5,4,3,2,1,0,3,9,6],v.as_slice());
	}
	#[test]
	fn extend_within(){
		let mut v=FileVec::new();

		v.extend_from_slice(&[5,4,3,2,1]);
		assert_eq!([5,4,3,2,1],v.as_slice());

		v.extend_from_within(1..4);
		assert_eq!([5,4,3,2,1,4,3,2],v.as_slice());
	}
	#[test]
	fn extract_if(){
		let mut v=FileVec::new();
		v.extend_from_slice(&[5,4,3,2,1,0,-2,-4,-6,3,3,3]);

		let extracted:Vec<i32>=v.extract_if(2..,|x|*x%2==0).take(3).collect();

		assert_eq!(extracted,[2,0,-2]);
		assert_eq!(v.as_slice(),[5,4,3,1,-4,-6,3,3,3]);

		let extracted:Vec<i32>=v.extract_if(2..,|x|*x%2>0).rev().take(3).collect();

		assert_eq!(extracted,[3,3,3]);
		assert_eq!(v.as_slice(),[5,4,3,1,-4,-6]);
	}
	#[test]
	fn insert_remove(){
		let mut v=FileVec::from([0,1,2,3,4,5,6,7,8,9]);

		v.insert(10,-9);
		v.insert( 9,-8);
		v.insert( 3,-2);

		v.insert( 9,-7);
		v.insert( 8,-6);
		v.insert( 7,-5);
		v.insert( 6,-4);
		v.insert( 5,-3);
		//v.insert( 3,-2);
		v.insert( 2,-1);
		v.insert( 1, 0);
		v.insert( 0, 1);

		assert_eq!(v.as_slice(),[1,0,0,1,-1,2,-2,3,-3,4,-4,5,-5,6,-6,7,-7,8,-8,9,-9]);

		assert_eq!(v.remove(0),1);
		assert_eq!(v.remove(1),0);
		assert_eq!(v.remove(1),1);
		assert_eq!(v.remove(2),2);
		assert_eq!(v.remove(9),6);

		assert_eq!(v.remove(3),3);
		assert_eq!(v.remove(4),4);
		assert_eq!(v.remove(5),5);
		//assert_eq!(v.remove(6),6);
		assert_eq!(v.remove(7),7);
		assert_eq!(v.remove(8),8);
		assert_eq!(v.remove(9),9);

		assert_eq!(v.as_slice(),[0,-1,-2,-3,-4,-5,-6,-7,-8,-9]);
		assert_eq!(v.remove(9),-9);
		assert_eq!(v.remove(8),-8);
		assert_eq!(v.remove(7),-7);
		assert_eq!(v.remove(6),-6);
	}
	#[test]
	fn load_persist(){
		let mut v=FileVec::new();

		v.push(5);
		v.push(4);
		v.push(3);
		v.push(2);
		v.push(1);
		v.set_persistent(true);

		let path=PathBuf::from(v.path().unwrap());

		mem::drop(v);
		v=unsafe{FileVec::open(path).unwrap()};
		v.set_persistent(false);

		assert_eq!([5,4,3,2,1],v.as_slice());
	}
	#[test]
	fn new_push(){
		let mut v=FileVec::new();

		v.push(5);
		v.push(4);
		v.push(3);
		v.push(2);
		v.push(1);

		assert_eq!([5,4,3,2,1],v.as_slice())
	}
	#[test]
	fn remove_range(){
		let mut v=FileVec::new();

		v.push(5);
		v.push(4);
		v.push(3);
		v.push(2);
		v.push(1);
		v.remove_range(..3);

		assert_eq!([2,1],v.as_slice());

		v.push(2);
		v.push(3);
		v.push(4);
		v.remove_range(2..4);

		assert_eq!([2,1,4],v.as_slice());

		v.remove_range(1..2);

		assert_eq!([2,4],v.as_slice());

		v.remove_range(1..1);

		assert_eq!([2,4],v.as_slice());

		v.push(6);
		v.push(8);
		v.push(10);
		v.push(11);
		v.push(12);
		v.remove_range(4..);

		assert_eq!([2,4,6,8],v.as_slice());

		v.remove_range(..);

		assert_eq!([0_i32;0],v.as_slice());
	}
	#[test]
	fn retain(){
		let mut v=FileVec::new();
		v.extend_from_slice(&[5,4,3,2,1,0,-2,-4,-6,3,3,3]);

		v.retain(|x|*x%2==0);
		assert_eq!(v.as_slice(),[4,2,0,-2,-4,-6]);
	}

	use std::{mem,path::PathBuf,sync::Arc};
	use super::*;
}
/// helper struct for running function tail code that is required for drop soundness even if a panic occurs
pub (crate) struct FinalizeDrop<F:FnOnce()>(MaybeUninit<F>);

pub mod iter;
pub mod vec;

pub use vec::FileVec;
use std::mem::MaybeUninit;
