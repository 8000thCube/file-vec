impl<E:Clone> Clone for FileVec<E>{
	fn clone(&self)->Self{
		let mut result=FileVec::new();

		result.extend_from_slice(&self);
		result.set_persistent(self.persistent);
		result
	}
	fn clone_from(&mut self,other:&Self){
		self.clear();
		self.extend_from_slice(other);
	}
}
impl<E:Clone> From<&[E]> for FileVec<E>{
	fn from(slice:&[E])->Self{slice.iter().cloned().collect()}
}
impl<E,const N:usize> FileVec<[E;N]>{
	/// flatten FileVec<[E;N]> to FileVec<E>
	pub fn into_flattened(mut self)->FileVec<E>{
		FileVec{
			len:mem::take(&mut self.len).checked_mul(N).unwrap(),
			map:self.map.take(),
			path:self.path.take(),
			persistent:mem::take(&mut self.persistent),
			phantom:PhantomData
		}
	}
}
impl<E,const N:usize> From<[E;N]> for FileVec<E>{
	fn from(slice:[E;N])->Self{slice.into_iter().collect()}
}
impl<E> AsMut<[E]> for FileVec<E>{
	fn as_mut(&mut self)->&mut [E]{self.as_mut_slice()}
}
impl<E> AsMut<Self> for FileVec<E>{
	fn as_mut(&mut self)->&mut Self{self}
}
impl<E> AsRef<[E]> for FileVec<E>{
	fn as_ref(&self)->&[E]{self.as_slice()}
}
impl<E> AsRef<Self> for FileVec<E>{
	fn as_ref(&self)->&Self{self}
}
impl<E> Borrow<[E]> for FileVec<E>{
	fn borrow(&self)->&[E]{self.as_slice()}
}
impl<E> BorrowMut<[E]> for FileVec<E>{
	fn borrow_mut(&mut self)->&mut [E]{self.as_mut_slice()}
}
impl<E> Deref for FileVec<E>{
	fn deref(&self)->&Self::Target{self.as_slice()}
	type Target=[E];
}
impl<E> DerefMut for FileVec<E>{
	fn deref_mut(&mut self)->&mut Self::Target{self.as_mut_slice()}
}
impl<E> Default for FileVec<E>{
	fn default()->Self{Self::new()}
}
impl<E> Drop for FileVec<E>{
	fn drop(&mut self){self.close()}
}
impl<E> Extend<E> for FileVec<E>{
	fn extend<I:IntoIterator<Item=E>>(&mut self,iter:I){			// for unknown iter sizes limit reserve to 10^10 bytes to avoid unhinged file resizes from iterators with more conservative size hints than their use case
		let iter=iter.into_iter();
		let unknownreservelimit=1_00000_00000/mem::size_of::<E>();
																	// reserve low if the hint appears exact, otherwise reserve high up to unknownreservelimit. ensure res is at least 1 because size_hint is not guaranteed correct for unsafe purposes. if the iter len actually is 0 for_each would immediately return and the resize in the loop wouldn't be executed
		let (low,high)=iter.size_hint();
		let mut cap=self.len;
		let mut p=0 as *mut E;
		let res=if Some(low)==high{low}else{high.unwrap_or(low).min(unknownreservelimit)}.max(1);

		iter.for_each(|x|unsafe{									// reserve being at least 1 ensures there's space for 1 more, although p was initialized null, cap was initialized to self.len so p will be set before use
			if cap<=self.len{										// if first iteration or capacity matches length, reserve space and update capacity
				self.reserve(res);
																	// if first iteration or capacity matches length, set the pointer to the end of the length in the current allocation
				cap=self.capacity();
				p=self.as_mut_ptr().add(self.len);
			}
																	// write the item to the end of the length, then increment length and pointer
			ptr::write(p,x);
			p=p.add(1);
			self.len+=1;
		});
	}
}
impl<E> FileVec<E>{
	/// allow len adjustment from other files in the crate
	pub (crate) unsafe fn len_mut(&mut self)->&mut usize{&mut self.len}
	/// moves all the elements of other into self, leaving other empty
	pub fn append(&mut self,other:&mut Self){
		assert_ne!(self.as_mut_ptr(),other.as_mut_ptr());
		self.reserve(other.len);
											// the base pointers are different if the file vec was initialized correctly, so i'd assume the files don't overlap so they shouldn't overlap in the memory map
		unsafe{ptr::copy_nonoverlapping(other.as_mut_ptr() as *const E,self.as_mut_ptr().add(self.len),other.len)}
		self.len+=other.len;
		other.len=0;
	}
	/// references the mapped memory up to self.len() as a slice
	pub fn as_mut_slice(&mut self)->&mut [E]{
		let bytes:&mut [u8]=if let Some(m)=&mut self.map{m}else{return &mut []};
		let items=unsafe{					// this should be okay if the file vec was created correctly and nothing modifed the file data
			bytes.align_to_mut::<MaybeUninit<E>>()
		};
											// mmap should align to os page size which should work for anything. assert just in case
		assert_eq!(items.0.len(),0);
		assert_eq!(items.2.len(),0);

		unsafe{								// the items up to len are initialized
			mem::transmute(&mut items.1[..self.len])
		}
	}
	/// returns a pointer to the start of the mapped memory
	pub fn as_mut_ptr(&mut self)->*mut E{self.as_mut_slice().as_mut_ptr()}
	/// returns a pointer to the start of the mapped memory
	pub fn as_ptr(&self)->*const E{self.as_slice().as_ptr()}
	/// references the mapped memory up to self.len() as a slice
	pub fn as_slice(&self)->&[E]{
		let bytes:&[u8]=if let Some(m)=&self.map{m}else{return &[]};
		let items=unsafe{					// this should be okay if the file vec was created correctly and nothing modifed the file data
			bytes.align_to::<MaybeUninit<E>>()
		};
											// mmap should align to os page size which should work for anything
		assert_eq!(items.0.len(),0);
		assert_eq!(items.2.len(),0);

		unsafe{								// the items up to len are initialized
			mem::transmute(&items.1[..self.len])
		}
	}
	/// returns the capacity of the file FileVec
	pub fn capacity(&self)->usize{self.map.as_ref().map(|x|x.len()/mem::size_of::<E>()).unwrap_or(0)}
	/// drops all the items and sets len to 0. use close to make the vec also use a fresh file next time items are added
	pub fn clear(&mut self){
		if mem::needs_drop::<E>(){
			let p=self.as_mut_ptr();
			while self.len>0{				// file vec keeps data initialized up to len
				self.len-=1;
				unsafe{ptr::drop_in_place(p.add(self.len))}
			}
		}else{
			self.len=0;
		}
	}
	/// closes the backing file and resets the path. The file will be deleted unless self.is_persistent() is true. The persistence flag is preserved. self.len() will be 0 after this returns
	pub fn close(&mut self){				// Take the path out of self.path to use and reset it. No need to close anything if None because there's no file. Shouldn't need to reset len in that case either because the items would need to be stored
		let l=self.len;
		let path=if let Some(p)=self.path.take(){p}else{return};
											// drop items that need drop and reset len to 0
		self.clear();
											// drop mmap handle
		self.map=None;
											// trim the file to length if persistent for compact storage and ability to infer correct length when opening. delete the file otherwise
		if self.persistent{
			let file=OpenOptions::new().create(true).write(true).open(&path).unwrap();
			file.set_len((mem::size_of::<E>()*l).try_into().unwrap()).unwrap();
		}else{
			fs::remove_file(&path).unwrap();
		}
	}
	/// dedup by partial eq
	pub fn dedup(&mut self) where E:PartialEq{self.dedup_by(|x,y|x==y)}
	/// remove adjacent duplicates according to the same bucket function
	pub fn dedup_by<F:FnMut(&mut E,&mut E)->bool>(&mut self,mut f:F){
		let remaining=Cell::new(self.len);
		if remaining.get()<=1{return}else{remaining.update(|r|r-1)}
										// create new length counter and read and write pointers
		let l:Cell<usize> =Cell::new(1);// a pointer to index 1 is valid because len is greater than 1. Otherwise the function would have returned early 2 lines ago
		let r:Cell<*mut E>=Cell::new(unsafe{self.as_mut_ptr().add(1)});
		let w:Cell<*mut E>=Cell::new(r.get());
										// finalize by updating moving remaining items if any, then updating self.len to reflect the new length
		let finalize=FinalizeDrop::new(||unsafe{
			let remainder=remaining.get();
			if remainder>0{				// if comparison or drop panic, move the rest of the array as if no further duplicates
				ptr::copy(r.get(),w.get(),remainder);
				l.update(|l|l+remainder);
			}
			self.len=l.get();
		});								// refill with deduplicated items. the read pointer stays at or ahead of the write pointer
		while remaining.get()>0{
			unsafe{						// extract references to current and previous items
				let current=&mut *r.get();
				let previous=&mut *r.get().sub(1);
										// check if duplicate
				let f=f(current,previous);
										// update r after f but before drop to ensure even in case of f panic or drop panic, the current item is removed if and only if f returns true
				r.update(|r|r.add(1));
				remaining.update(|r|r-1);
										// if f is true, drop the current item, otherwise, move it to the current write position
				if f{
					ptr::drop_in_place(r.get());
				}else{
					ptr::copy(current,w.get(),1);
										// update new length and write pointer after moving
					l.update(|l|l+1);
					w.update(|w|w.add(1));
				}
			}
		}
										// finalize
		mem::drop(finalize);
	}
	/// dedup by key
	pub fn dedup_by_key<F:FnMut(&mut E)->K,K:PartialEq>(&mut self,mut f:F){self.dedup_by(|x,y|f(x)==f(y))}
	/// drain a range of items. Panics if start>stop or either start or stop is greater than self.len()
	pub fn drain<R:RangeBounds<usize>>(&mut self,range:R)->Drain<'_,E>{Drain::new(self,range)}
	/// pushes clones of all the items in the slice onto the file vec
	pub fn extend_from_slice(&mut self,slice:&[E]) where E:Clone{
		let l=slice.len();
		self.reserve(l);
										// get pointers
		let mut p=unsafe{self.as_mut_ptr().add(self.len)};
		let mut s=slice.as_ptr();
										// iterate over the length of the slice, cloning slice items onto the vec. the length of the slice was reserved, so we wont run out of space
		for _ in 0..l{
			unsafe{
				ptr::write(p,s.as_ref().unwrap_unchecked().clone());
				self.len+=1;
										// increment pointers after writing
				p=p.add(1);
				s=s.add(1);
			}
		}
	}
	/// clone a range of items onto the end. Panics if start>stop or either start or stop is greater than self.len()
	pub fn extend_from_within<R:RangeBounds<usize>>(&mut self,range:R) where E:Clone{
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
			Bound::Unbounded   =>self.len
		};
														// bounds check
		assert!(start<=self.len);
		assert!(start<=stop);
		assert!(stop <=self.len);
														// reserve the needed space
		self.reserve(stop-start);
		let v=self.as_mut_ptr();
														// get pointers
		let mut p=unsafe{v.add(self.len)};
		let mut s=unsafe{v.add(start)};
														// iterate over the length of the slice, cloning slice items onto the vec. the length of the slice was reserved, so we wont run out of space
		for _ in 0..stop-start{
			unsafe{
				ptr::write(p,s.as_ref().unwrap_unchecked().clone());
				self.len+=1;
														// increment pointers after writing
				p=p.add(1);
				s=s.add(1);
			}
		}
	}
	/// removes and yields items from the range where f is true. if the iterator is not exhausted, the remaining items will be left in the collection
	pub fn extract_if<R:RangeBounds<usize>,F:FnMut(&mut E)->bool>(&mut self,r:R,f:F)->ExtractIf<'_,E,F>{ExtractIf::new(self,r,f)}
	/// Creates a FileVec<T> where each element is produced by calling f with that element’s index while walking forward through the FileVec<T>
	pub fn from_fn<F:FnMut(usize)->E>(length:usize,f:F)->Self{(0..length).map(f).collect()}
	/// insert the item at the index, shifting all items after it
	pub fn insert(&mut self,index:usize,item:E){
		self.insert_mut(index,item);
	}
	/// insert the item at the index, shifting all items after it
	pub fn insert_mut(&mut self,index:usize,item:E)->&mut E{
		assert!(index<=self.len);
		self.reserve(1);
		unsafe{											// index is in bounds and should have at least one space available to copy to
			let p=self.as_mut_ptr().add(index);
														// copy everything at or above p to one index higher, then write item to p
			ptr::copy(p,p.add(1),self.len-index);
			ptr::write(p,item);
														// adjust len and return the reference
			self.len+=1;
			p.as_mut().unwrap_unchecked()
		}
	}
	/// groups every N items into chunks, dropping the remainder
	pub fn into_chunks<const N:usize>(mut self)->FileVec<[E;N]>{
		self.truncate(self.len-self.len%N);

		FileVec{
			len:mem::take(&mut self.len)/N,
			map:self.map.take(),
			path:self.path.take(),
			persistent:mem::take(&mut self.persistent),
			phantom:PhantomData
		}
	}
	/// checks if empty
	pub fn is_empty(&self)->bool{self.len==0}
	/// returns whether the backing file will persist after the vec is dropped
	pub fn is_persistent(&self)->bool{self.persistent}
	/// returns the length of the file vec
	pub fn len(&self)->usize{self.len}
	/// creates new file vec. The file won't be created and the vec won't allocate until items are added to it
	pub fn new()->Self{
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0);

		Self{
			len:0,
			map:None,
			path:None,
			persistent:false,
			phantom:PhantomData
		}
	}
	/// references the file path. returns none if the vec is new and empty and hasn't created a file yet
	pub fn path(&self)->Option<&Path>{self.path.as_deref()}
	/// remove the last item from the file
	pub fn pop(&mut self)->Option<E>{
		let l=self.len;
		if l==0{return None}

		self.len=l-1;
		Some(unsafe{ptr::read(self.as_mut_ptr().add(l-1))})
	}
	/// conditionally remove the last item from the file
	pub fn pop_if<F:FnOnce(&mut E)->bool>(&mut self,f:F)->Option<E>{
		let l=self.len;
		if l==0{return None}

		unsafe{
			let p=self.as_mut_ptr().add(l-1);
			if !f(p.as_mut().unwrap_unchecked()){return None}

			self.len=l-1;
			Some(ptr::read(p))
		}
	}
	/// push an item onto the file vec
	pub fn push(&mut self,item:E){
		self.reserve(1);
		//dbg!((self.capacity(),self.len()));
		unsafe{ptr::write(self.as_mut_ptr().add(self.len),item)}

		self.len+=1;
	}
	/// push an item onto the file vec
	pub fn push_mut(&mut self,item:E)->&mut E{
		self.reserve(1);
		unsafe{
			let p=self.as_mut_ptr().add(self.len);
			ptr::write(p,item);

			self.len+=1;
			p.as_mut().unwrap_unchecked()
		}
	}
	/// push an item onto the file vec
	pub fn push_within_capacity(&mut self,item:E)->Result<&mut E,E>{
		if self.len()<self.capacity(){
			Ok(unsafe{
				let p=self.as_mut_ptr().add(self.len);
				ptr::write(p,item);

				self.len+=1;
				p.as_mut().unwrap_unchecked()
			})
		}else{
			Err(item)
		}
	}
	/// remove and return the item at the index and shift elements after it down 1
	pub fn remove(&mut self,index:usize)->E{
		let l=self.len;
		assert!(index<l);

		unsafe{
			let p=self.as_mut_ptr().add(index);
			let item=ptr::read(p);

			self.len=l-1;
			ptr::copy(p.add(1),p,l-index);
			item
		}
	}
	/// remove and drop a range of items. Panics if start>stop or either start or stop is greater than self.len()
	pub fn remove_range<R:RangeBounds<usize>>(&mut self,range:R){
		Drain::new(self,range);
	}
	/// reserves capacity for at least additional more items in the backing file
	pub fn reserve(&mut self,additional:usize){
		let required=additional.saturating_add(self.len());
		assert!(required<isize::MAX as usize);
														// check if necessary and compute next capacity
		let newcapacity=if required<=self.capacity(){return}else{required.next_power_of_two()};
		if self.path.is_none(){							// generate a path if not allocated yet
			let uid:u64=rand::random();
			self.path=Some(format!(".file-vec_{uid:x}").into());
		}
														// unmap and reopen the file
		self.map=None;
		let file=OpenOptions::new().create(true).read(true).write(true).open(self.path.as_ref().unwrap()).unwrap();
														// extend file length and remap
		file.set_len((mem::size_of::<E>()*newcapacity).try_into().unwrap()).unwrap();
		unsafe{self.map=Some(MmapMut::map_mut(&file).unwrap())}
	}
	/// reserves capacity for at least additional more items in the backing file
	pub fn reserve_exect(&mut self,additional:usize){
		let required=additional.saturating_add(self.len());
		assert!(required<isize::MAX as usize);
														// check if necessary and compute next capacity
		let newcapacity=if required<=self.capacity(){return}else{required};
		if self.path.is_none(){							// generate a path if not allocated yet
			let uid:u64=rand::random();
			self.path=Some(format!(".file-vec_{uid:x}").into());
		}
														// unmap and reopen the file
		self.map=None;
		let file=OpenOptions::new().create(true).read(true).write(true).open(self.path.as_ref().unwrap()).unwrap();
														// extend file length and remap
		file.set_len((mem::size_of::<E>()*newcapacity).try_into().unwrap()).unwrap();
		unsafe{self.map=Some(MmapMut::map_mut(&file).unwrap())}
	}
	/// resize
	pub fn resize(&mut self,len:usize,val:E) where E:Clone{
		if len<self.len{return self.truncate(len)}else if len==self.len{return};

		self.extend((self.len..len-1).map(|_|val.clone()));
		self.push(val);
	}
	/// resize
	pub fn resize_with<F:FnMut()->E>(&mut self,len:usize,mut f:F){
		if len<self.len{return self.truncate(len)}else if len==self.len{return};
		self.extend((self.len..len).map(|_|f()))
	}
	/// remove all items for which the function returns false
	pub fn retain<F:FnMut(&E)->bool>(&mut self,mut f:F){self.retain_mut(|x|f(x))}
	/// remove all items for which the function returns false
	pub fn retain_mut<F:FnMut(&mut E)->bool>(&mut self,mut f:F){
		let remaining=Cell::new(self.len);
		if remaining.get()==0{return}
										// create new length counter and read and write pointers
		let l:Cell<usize> =Cell::new(0);
		let r:Cell<*mut E>=Cell::new(self.as_mut_ptr());
		let w:Cell<*mut E>=Cell::new(r.get());
										// finalize by updating moving remaining items if any, then updating self.len to reflect the new length
		let finalize=FinalizeDrop::new(||unsafe{
			let remainder=remaining.get();
			if remainder>0{				// if comparison or drop panic, move the rest of the array as if no further removals
				ptr::copy(r.get(),w.get(),remainder);
				l.update(|l|l+remainder);
			}
			self.len=l.get();
		});								// refill with retained items. the read pointer stays at or ahead of the write pointer
		while remaining.get()>0{
			unsafe{						// extract references to current item and check if retained
				let current=&mut *r.get();
				let f=f(current);
										// update r after f but before drop to ensure even in case of f panic or drop panic, the current item is removed if and only if f returns false
				r.update(|r|r.add(1));
				remaining.update(|r|r-1);
										// if f is true, drop the current item, otherwise, move it to the current write position
				if f{
					ptr::copy(current,w.get(),1);
										// update new length and write pointer after moving
					l.update(|l|l+1);
					w.update(|w|w.add(1));
				}else{
					ptr::drop_in_place(r.get());
				}
			}
		}
										// finalize
		mem::drop(finalize);
	}
	/// sets whether the backing file should persist after the vec is dropped. Technically the file just existing doesn't cause any soundness problems, so this isn't marked as unsafe, but it probably shouldn't be set true on types requiring drop if the intent is to open the file in a new file vec later. Default value is usually false, except it's true for FileVec created by opening an existing file
	pub fn set_persistent(&mut self,persistent:bool){self.persistent=persistent}
	/// shrinks the capacity of the vector with a lower bound
	pub fn shrink_to(&mut self,mut mincap:usize){
		if mincap>=self.capacity(){return}
		if mincap<self.len{mincap=self.len}
											// get file path and unmap so we can edit the file through the file system
		let path=if let Some(p)=self.path.take(){p}else{return};
		self.map=None;
											// trim the file to length
		let file=OpenOptions::new().create(true).write(true).open(&path).unwrap();
		file.set_len((mem::size_of::<E>()*mincap).try_into().unwrap()).unwrap();
											// remap
		self.path=Some(path);
		unsafe{self.map=Some(MmapMut::map_mut(&file).unwrap())}
	}
	/// Shrinks the capacity of the vector as much as possible.
	pub fn shrink_to_fit(&mut self){self.shrink_to(self.len)}
	/// Returns the remaining spare capacity of the vector as a slice of MaybeUninit<E>
	pub fn spare_capacity_mut(&mut self)->&mut [MaybeUninit<E>]{self.split_at_spare_mut().1}
	/// Returns vector content as a slice of T, along with the remaining spare capacity of the vector as a slice of MaybeUninit<T>.
	pub fn split_at_spare_mut(&mut self)->(&mut [E],&mut [MaybeUninit<E>]){
		let bytes:&mut [u8]=if let Some(m)=&mut self.map{m}else{return (&mut [],&mut [])};
		let items=unsafe{					// this should be okay if the file vec was created correctly and nothing modifed the file data
			bytes.align_to_mut::<MaybeUninit<E>>()
		};
											// mmap should align to os page size which should work for anything. assert just in case
		assert_eq!(items.0.len(),0);
		assert_eq!(items.2.len(),0);

		unsafe{								// the items up to len are initialized
			mem::transmute(items.1.split_at_mut(self.len))
		}
	}
	/// split the collection in two at the given index
	pub fn split_off(&mut self,a:usize)->Self{self.drain(a..).collect()}
	/// returns the item at this index, replacing it with the last item
	pub fn swap_remove(&mut self,index:usize)->E{
		assert!(index<self.len);

		let last=self.pop().unwrap();
		mem::replace(&mut self[index],last)
	}
	/// shorten the FileVec length to n if it's longer, dropping the extra
	pub fn truncate(&mut self,n:usize){
		if mem::needs_drop::<E>(){
			unsafe{
				let p=self.as_mut_ptr();
				while n<self.len{
					self.len-=1;
					ptr::drop_in_place(p.add(self.len));
				}
			}
		}else{
			self.len=self.len.min(n);
		}
	}
	/// create a file vec with at least an initial capacity
	pub fn with_capacity(cap:usize)->Self{
		let mut result=Self::new();

		result.reserve(cap);
		result
	}
	/// opens a file as a FileVec. The bytes in the file must be valid when interpreted as [T], the file should not be modified while in use, and the file must not be open in any other file vec or other memory mapping. An empty file will be created when the file vec allocates if the file doesn't exist. Regardless of whether the file already existed, the FileVec's persistence flag will be initialized to true
	pub unsafe fn open<P:AsRef<Path>>(path:P)->IOResult<Self>{
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0);

		let path:PathBuf=path.as_ref().into();
		let persistent=true;
		let phantom=PhantomData;

		let map=match OpenOptions::new().read(true).write(true).open(&path){
			Err(e)=>if e.kind()==IOErrorKind::NotFound{
				None
			}else{
				return Err(e)
			},
			Ok(file)=>Some(unsafe{MmapMut::map_mut(&file)?}),
		};

		let len=map.as_ref().map(|x|x.len()/mem::size_of::<E>()).unwrap_or(0);
		let path=Some(path);

		Ok(Self{len,map,path,persistent,phantom})
	}
	/// sets the length of the file vec. The data must be initialized up to the new length, and the new length must be within capacity
	pub unsafe fn set_len(&mut self,len:usize){self.len=len}
	/// sets the path to store the data in. The old file will be deleted unless the persistence flag is true. If data is already in the file, it will be copied to the file at the new path. If the new path points to an existing file it will be overwritten. The file should not be modified while in use, and the file must not be open in any other file vec or other memory mapping.
	pub unsafe fn set_path<P:AsRef<Path>>(&mut self,path:P)->IOResult<()>{
		if let Some(p)=self.path(){
			if p==path.as_ref(){return Ok(())}
		}

		let old=self.path.clone();
		let persistent=self.is_persistent();
											// close the file and leave it in the file system so we can copy it
		self.set_persistent(true);
		self.close();
											// copy the old file to the new file and remove if not persistent
		if let Some(old)=old{
			fs::copy(&old,&path)?;
			if !persistent{fs::remove_file(old)?}
		}
											// open the new file as a file vec and preserve the persistence flag
		*self=unsafe{Self::open(path)?};
		self.set_persistent(persistent);

		Ok(())
	}
}
impl<E> FromIterator<E> for FileVec<E>{
	fn from_iter<I:IntoIterator<Item=E>>(collection:I)->Self{
		let mut result=Self::new();

		result.extend(collection);
		result
	}
}

#[derive(Debug)]
/// memory maps a file into a vec like structure. Avoid modifying the backing file while the file vec is living. Item alignment must be a factor of os page size. Cloning will copy the file. ZST currently not supported
pub struct FileVec<E>{len:usize,map:Option<MmapMut>,path:Option<PathBuf>,persistent:bool,phantom:PhantomData<E>}

use crate::{
	FinalizeDrop,iter::{Drain,ExtractIf}
};
use memmap2::MmapMut;
use std::{
	borrow::{Borrow,BorrowMut},cell::Cell,cmp::PartialEq,fs::{OpenOptions,self},io::{ErrorKind as IOErrorKind,Result as IOResult},iter::FromIterator,marker::PhantomData,mem::{MaybeUninit,self},ops::{Bound,Deref,DerefMut,RangeBounds},path::{PathBuf,Path},ptr
};
