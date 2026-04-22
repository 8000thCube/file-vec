impl<'a,E,F:FnMut(&mut E)->bool> Drop for ExtractIf<'a,E,F>{
	fn drop(&mut self){									// vec structure like (head: 0..pw, pgap: pw..pr, remaining: pr..qr, qgap: qr..qw, tail: qw..len)
		unsafe{											// bounds were checked on construction for soundness, and presumably remained sound
			let (pr,pw)=(self.pr,self.pw);
			let (qr,qw)=(self.qr,self.qw);
			let items=self.items;
			let len=self.len;
														// compute gap lengths and total items removed by the difference between the read and write pointers
			let (pg,qg)=(pr.offset_from(pw) as usize,qw.offset_from(qr) as usize);
			let remaining=qr.offset_from(pr) as usize;
			let totalremoved=pg+qg;
														// fill the two gaps from forward and reverse iteration
			ptr::copy(pr,pw,remaining);
			ptr::copy(qw,pw.add(remaining),items.add(*len).offset_from(qw) as usize);
														// adjust len
			*len-=totalremoved;
		}
	}
}
impl<'a,E,F:FnMut(&mut E)->bool> DoubleEndedIterator for ExtractIf<'a,E,F>{
	fn next_back(&mut self)->Option<Self::Item>{
		let (mut qr,mut qw)=(self.qr,self.qw);
		let pr=self.pr;

		let f=&mut self.f;
		let item=loop{
			unsafe{
				let mut item=if pr<qr{ptr::read(qr.offset(-1))}else{break None};
				qr=qr.offset(-1);

				if f(&mut item){break Some(item)}
				ptr::write(qw.offset(-1),item);
				qw=qw.offset(-1);
			}
		};

		(self.qr,self.qw)=(qr,qw);
		item
	}
}
impl<'a,E,F:FnMut(&mut E)->bool> ExtractIf<'a,E,F>{
	/// create a new extract if from the file
	pub fn new<R:RangeBounds<usize>>(vec:&'a mut FileVec<E>,range:R,f:F)->Self{
		let begin=range.start_bound();
		let end=range.end_bound();
		let start=match begin{							// normalize bounds
			Bound::Excluded(&n)=>n.saturating_add(1),
			Bound::Included(&n)=>n,
			Bound::Unbounded   =>0
		};
		let stop= match end{
			Bound::Excluded(&n)=>n,
			Bound::Included(&n)=>n.saturating_add(1),
			Bound::Unbounded   =>vec.len()
		};
														// bounds check
		assert!(start<=vec.len());
		assert!(start<=stop);
		assert!(stop <=vec.len());

		let marker=PhantomData;
		let len=unsafe{vec.len_mut() as *mut usize};
		let items=vec.as_mut_ptr();

		let pr=unsafe{items.add(start)};
		let qr=unsafe{items.add(stop)};

		let (pw,qw)=(pr,qr);

		Self{f,items,len,marker,pr,pw,qr,qw}
	}
}
impl<'a,E,F:FnMut(&mut E)->bool> Iterator for ExtractIf<'a,E,F>{
	fn next(&mut self)->Option<Self::Item>{
		let (mut pr,mut pw)=(self.pr,self.pw);
		let qr=self.qr;

		let f=&mut self.f;
		let item=loop{
			unsafe{
				let mut item=if pr<qr{ptr::read(pr)}else{break None};
				pr=pr.add(1);

				if f(&mut item){break Some(item)}
				ptr::write(pw,item);
				pw=pw.add(1);
			}
		};

		(self.pr,self.pw)=(pr,pw);
		item
	}
	fn size_hint(&self)->(usize,Option<usize>){
		let remaining=unsafe{self.qr.offset_from(self.pr) as usize};
		(0,Some(remaining))
	}
	type Item=E;
}
impl<'a,E> AsMut<[E]> for Drain<'a,E>{
	fn as_mut(&mut self)->&mut [E]{self.as_mut_slice()}
}
impl<'a,E> AsMut<Self> for Drain<'a,E>{
	fn as_mut(&mut self)->&mut Self{self}
}
impl<'a,E> AsRef<[E]> for Drain<'a,E>{
	fn as_ref(&self)->&[E]{self.as_slice()}
}
impl<'a,E> AsRef<Self> for Drain<'a,E>{
	fn as_ref(&self)->&Self{self}
}
impl<'a,E> Borrow<[E]> for Drain<'a,E>{
	fn borrow(&self)->&[E]{self.as_slice()}
}
impl<'a,E> BorrowMut<[E]> for Drain<'a,E>{
	fn borrow_mut(&mut self)->&mut [E]{self.as_mut_slice()}
}
impl<'a,E> Deref for Drain<'a,E>{
	fn deref(&self)->&Self::Target{self.as_slice()}
	type Target=[E];
}
impl<'a,E> DerefMut for Drain<'a,E>{
	fn deref_mut(&mut self)->&mut Self::Target{self.as_mut_slice()}
}
impl<'a,E> DoubleEndedIterator for Drain<'a,E>{
	fn next_back(&mut self)->Option<E>{
		let q=self.q;

		if q<=self.p{return None}
		unsafe{
			let item=ptr::read(q.offset(-1));
			self.q=q.offset(-1);

			Some(item)
		}
	}
	fn nth_back(&mut self,n:usize)->Option<E>{
		if mem::needs_drop::<E>(){
			for item in self.rev().take(n){mem::drop(item)}
		}else if n<self.len(){
			unsafe{					// p isn't allowed past q, but since the slice is from p to q, p+n<q is guaranteed by n being less than len
				self.q=self.q.offset(-(n as isize))
			}
		}
		self.next_back()
	}
}
impl<'a,E> Drain<'a,E>{
	/// reference the remaining items
	pub fn as_slice(&self)->&[E]{
		unsafe{
			let len=self.q.offset_from(self.p) as usize;
			let p=self.p;

			slice::from_raw_parts(p,len)
		}
	}
	/// reference the remaining items
	pub fn as_mut_slice(&mut self)->&mut [E]{
		unsafe{
			let len=self.q.offset_from(self.p) as usize;
			let p=self.p;

			slice::from_raw_parts_mut(p,len)
		}
	}
	/// clear the drain, returning the remaining items
	pub fn keep_rest(&mut self){
		unsafe{											// bounds were checked on construction for soundness, and presumably remained sound
			let (start,stop)=(self.start,self.stop);
			let items=self.items;
			let len=self.len.as_mut().unwrap_unchecked();
			let remaining=self.q.offset_from(self.p) as usize;
			let pstart=items.add(start);
			let pstop= items.add(stop);
														// copy items remaining in the range to the start of the drained area, copy items after the range to fill the gap
			ptr::copy(self.p,pstart,remaining);
			ptr::copy(pstop,pstart.add(remaining),*len-stop);
														// adjust len and make the iteration pointers equal so self thinks it's an empty range and doesn't need to drop anything
			*len-=stop-start-remaining;
			self.p=self.q;
			self.start=self.stop;
		}
	}
	/// create a new drain structure from the file vec
	pub fn new<R:RangeBounds<usize>>(file:&'a mut FileVec<E>,range:R)->Self{
		let begin=range.start_bound();
		let end=range.end_bound();
		let start=match begin{							// normalize bounds
			Bound::Excluded(&n)=>n.saturating_add(1),
			Bound::Included(&n)=>n,
			Bound::Unbounded   =>0
		};
		let stop= match end{
			Bound::Excluded(&n)=>n,
			Bound::Included(&n)=>n.saturating_add(1),
			Bound::Unbounded   =>file.len()
		};
														// bounds check
		assert!(start<=file.len());
		assert!(start<=stop);
		assert!(stop <=file.len());

		let marker=PhantomData;
		let len=unsafe{file.len_mut() as *mut usize};
		let items=file.as_mut_ptr();

		let p=unsafe{items.add(start)};
		let q=unsafe{items.add(stop)};

		Self{items,len,marker,p,q,start,stop}
	}
}
impl<'a,E> Drop for Drain<'a,E>{
	fn drop(&mut self){
		unsafe{											// bounds were checked on construction for soundness, and presumably remained sound
			let (start,stop)=(self.start,self.stop);
			if start==stop{return}						// no need to do anything with an empty range. This usually works implicitly; the explicit edge case is needed when the results of keep_rest aren't compatible with the normal drop process. In this situation, the keep_rest function handles the drop and sets start=stop.
														// finalize by copy items after the range to fill the gap, then adjust the length
			let finalize=FinalizeDrop::new(||{
				let (items,len)=(self.items,self.len);
														// the tail (items after removal range) are copied so it starts where the removal range started
				ptr::copy(items.add(stop),items.add(start),*len-stop);
				*len-=stop-start;
			});
			if mem::needs_drop::<E>(){
				let mut p=self.p;
				let     q=self.q;
														// drop remaining items in the range if needed
				while p<q{
					ptr::drop_in_place(p);
					p=p.add(1);
				}
			}											// finalize
			mem::drop(finalize);
		}
	}
}
impl<'a,E> ExactSizeIterator for Drain<'a,E>{
	fn len(&self)->usize{
		unsafe{self.q.offset_from(self.p) as usize}
	}
}
impl<'a,E> FusedIterator for Drain<'a,E>{}
impl<'a,E> Iterator for Drain<'a,E>{
	fn next(&mut self)->Option<E>{
		let p=self.p;

		if p>=self.q{return None}
		unsafe{
			let item=ptr::read(p);
			self.p=p.add(1);

			Some(item)
		}
	}
	fn nth(&mut self,n:usize)->Option<E>{
		if mem::needs_drop::<E>(){
			for item in self.take(n){mem::drop(item)}
		}else if n<self.len(){
			unsafe{					// p isn't allowed past q, but since the slice is from p to q, p+n<q is guaranteed by n being less than len
				self.p=self.p.add(n)
			}
		}
		self.next()
	}
	fn size_hint(&self)->(usize,Option<usize>){
		let len=self.len();
		(len,Some(len))
	}
	type Item=E;
}

#[derive(Debug)]
/// file vec drain structure
pub struct Drain<'a,E>{items:*mut E,len:*mut usize,marker:PhantomData<&'a mut FileVec<E>>,p:*mut E,q:*mut E,start:usize,stop:usize}
#[derive(Debug)]
/// file vec extract if structure
pub struct ExtractIf<'a,E,F:FnMut(&mut E)->bool>{f:F,items:*mut E,len:*mut usize,marker:PhantomData<&'a mut FileVec<E>>,pr:*mut E,pw:*mut E,qr:*mut E,qw:*mut E}

unsafe impl<'a,E:Send,F:FnMut(&mut E)->bool+Send> Send for ExtractIf<'a,E,F>{}
unsafe impl<'a,E:Send> Send for Drain<'a,E>{}
unsafe impl<'a,E:Sync,F:FnMut(&mut E)->bool+Sync> Sync for ExtractIf<'a,E,F>{}
unsafe impl<'a,E:Sync> Sync for Drain<'a,E>{}

use crate::{FileVec,FinalizeDrop};
use std::{
	borrow::{Borrow,BorrowMut},iter::FusedIterator,marker::PhantomData,mem,ops::{Bound,Deref,DerefMut,RangeBounds},ptr,slice
};
