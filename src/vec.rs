fn generate_path()->PathBuf{format!(".file-vec_{:x}",rand::random::<u64>()).into()}

impl OnClose{
	/// construct a closebehavior of serialize from a path ref
	pub fn serialize<P:AsRef<Path>>(path:P)->Self{Self::Serialize(path.as_ref().to_path_buf().into())}
}
impl<E:Clone> Clone for FileVec<E>{
	fn clone(&self)->Self{
		let mut result=FileVec::new();

		result.clone_from(self);
		result
	}
	fn clone_from(&mut self,other:&Self){
		self.clear();
		self.closebehavior.clone_from(&other.closebehavior);
		self.extend_from_slice(other);
		#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
		{self.serialbehavior=other.serialbehavior.clone()}
	}
}
impl<E:Clone> From<&[E]> for FileVec<E>{
	fn from(slice:&[E])->Self{slice.iter().cloned().collect()}
}
impl<E,const N:usize> FileVec<[E;N]>{
	/// flatten FileVec<[E;N]> to FileVec<E>. if the close behavior is serialize, note that this does not preserve serial behavior
	pub fn into_flattened(mut self)->FileVec<E>{
		//if mem::size_of::<E>()==0{return FileVec::zst(self.len,self.path())}
		if self.buffer.is_none(){return FileVec::new()}

		let closebehavior=mem::replace(&mut self.closebehavior,OnClose::Persist);
		let path=self.buffer.take().unwrap().into_path();

		self.close();
		let mut result=unsafe{FileVec::open(path).unwrap()};

		/*#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]{
			result.serialbehavior=self.serialbehavior.take().map(|b|unsafe{
				let b:ComponentSB<Arc<dyn SerialBehavior<[E;N],BufReader<File>,BufWriter<File>>>,N>=ComponentSB(b);
				let a:Arc<dyn SerialBehavior<E,BufReader<File>,BufWriter<File>>>=Arc::new(b);
						// serial behavior is only some if static
				let p:*const  dyn         SerialBehavior<E,BufReader<File>,BufWriter<File>> =Arc::into_raw(a);
				let p:*const (dyn 'static+SerialBehavior<E,BufReader<File>,BufWriter<File>>)=mem::transmute_copy(&p);

				Arc::from_raw(p)
			});
		}*/

		result.closebehavior=closebehavior;
		result
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
impl<E> Buffer<E>{
	/// references the mapped memory as a slice
	pub fn as_mut_slice(&mut self)->&mut [MaybeUninit<E>]{
		let bytes:&mut [u8]=&mut self.map;
		let items=unsafe{					// this should be okay if the file vec was created correctly and nothing modifed the file data
			bytes.align_to_mut::<MaybeUninit<E>>()
		};
		// mmap should align to os page size which should work for anything. assert just in case
		assert_eq!(items.0.len(),0);
		assert_eq!(items.2.len(),0);

		items.1
	}
	/// references the mapped memory as a slice
	pub fn as_slice(&self)->&[MaybeUninit<E>]{
		let bytes:&[u8]=&self.map;
		let items=unsafe{					// this should be okay if the file vec was created correctly and nothing modifed the file data
			bytes.align_to::<MaybeUninit<E>>()
		};
		// mmap should align to os page size which should work for anything. assert just in case
		assert_eq!(items.0.len(),0);
		assert_eq!(items.2.len(),0);

		items.1
	}
	/// get the buffer capacity in number of components
	fn cap(&self)->usize{self.as_slice().len()}		// go through as slice to fail immediately and obviously if page size assumption is wrong
	/// unmap and convert into a path
	fn into_path(self)->PathBuf{self.path}
	/// create a new file buffer, creating or overwriting an existing file
	fn new(path:impl Into<Option<PathBuf>>,cap:usize)->IOResult<Self>{
		assert!(cap<=isize::MAX as usize/mem::size_of::<E>());
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0,"FileVec of zero sized type is currently not supported");
		// generate file
		let phantom=PhantomData;
		let path=path.into().unwrap_or_else(generate_path);
		let file=OpenOptions::new().create(true).read(true).write(true).open(&path)?;
		// map with appropriate len
		file.set_len((mem::size_of::<E>()*cap).try_into().unwrap())?;
		let map=unsafe{MmapMut::map_mut(&file)?};
		// construct result
		Ok(Self{map,path,phantom})
	}
	/// open a file as a file buffer, using the data inside if it already exists
	fn open(path:PathBuf)->IOResult<Self>{
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0,"FileVec of zero sized type is currently not supported");
		// open file
		let file=OpenOptions::new().create(true).read(true).write(true).open(&path)?;
		let map=unsafe{MmapMut::map_mut(&file)?};
		let phantom=PhantomData;
		// construct result
		Ok(Self{map,path,phantom})
	}
	/// set the file path of the file buffer, usually by renaming. the path must not be in use in any other memory mapping
	unsafe fn with_path(self,path:PathBuf)->IOResult<Self>{
		if path==self.path{return Ok(self)}
		mem::drop(self.map);
		fs::rename(&self.path,&path)?;
		// open the new path
		let file=OpenOptions::new().create(true).read(true).write(true).open(&path)?;
		let map=unsafe{MmapMut::map_mut(&file)?};
		let phantom=PhantomData;
		// construct result
		Ok(Self{map,path,phantom})
	}
	/// resize the buffer in number of components
	fn with_size(mut self,cap:usize)->IOResult<Self>{
		assert!(cap<=isize::MAX as usize/mem::size_of::<E>());
		// return early if size already matches
		if cap==self.cap(){return Ok(self)}
		mem::drop(self.map);
		// unmap and adjust file len
		let file=OpenOptions::new().create(true).read(true).write(true).open(&self.path)?;
		file.set_len((mem::size_of::<E>()*cap).try_into().unwrap())?;
		// construct result
		self.map=unsafe{MmapMut::map_mut(&file)?};
		Ok(self)
	}
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
	fn extend<I:IntoIterator<Item=E>>(&mut self,iter:I){			// for unknown iter sizes limit reserve to 10^10 bytes to avoid unhinged file resizes from iterators with size hints more conservative than their use case
		let iter=iter.into_iter();
		let unknownreservelimit=1_00000_00000/mem::size_of::<E>();

		let (low,high)=iter.size_hint();
		let mut cap=self.len;
		let mut p=0 as *mut E;

		iter.for_each(|x|unsafe{									// reserve at least 1 ensures there's space for 1 more/ although p was initialized null, cap was initialized to self.len so p will be set before use
			if cap<=self.len{										// if first iteration or capacity matches length, reserve space and update capacity. reserve exact if the hint appears exact. otherwise reserve high up to unknown reserve limit
				if Some(low)==high&&low>0{self.reserve_exact(low)}
				else{self.reserve(high.unwrap_or(unknownreservelimit).clamp(1,unknownreservelimit))}
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
		let b=if let Some(b)=&mut self.buffer{b.as_mut_slice()}else{&mut []};
		unsafe{								// the items up to len are initialized
			mem::transmute(&mut b[..self.len])
		}
	}
	/// returns a pointer to the start of the mapped memory
	pub fn as_mut_ptr(&mut self)->*mut E{self.as_mut_slice().as_mut_ptr()}
	/// returns a pointer to the start of the mapped memory
	pub fn as_ptr(&self)->*const E{self.as_slice().as_ptr()}
	/// references the mapped memory up to self.len() as a slice
	pub fn as_slice(&self)->&[E]{
		let b=if let Some(b)=&self.buffer{b.as_slice()}else{&[]};
		unsafe{								// the items up to len are initialized
			mem::transmute(&b[..self.len])
		}
	}
	/// returns the capacity of the file FileVec
	pub fn capacity(&self)->usize{self.buffer.as_ref().map(Buffer::cap).unwrap_or(0)}// yes this is items rather than bytes
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
	/// closes the backing file and resets the path after invoking the closing behavior. self.len() will be 0 after this returns, and self.path() will be None
	pub fn close(&mut self){
		if self.buffer.is_none(){
			self.len=0;
			return;
		}
		match &self.closebehavior{
			OnClose::Delete   =>{			// drop items that need drop and reset len to 0 before unmapping and deleting
				self.clear();
				fs::remove_file(self.buffer.take().unwrap().into_path()).unwrap();
			},
			OnClose::Persist  =>{			// trim the file to length if persistent for compact storage and ability to infer correct length when opening.
				let len=self.len();
				self.clear();

				let file=OpenOptions::new().create(true).write(true).open(self.buffer.take().unwrap().into_path()).unwrap();
				file.set_len((mem::size_of::<E>()*len).try_into().unwrap()).unwrap();
			},
			#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
			OnClose::Serialize(store)=>{
				let behavior=self.serialbehavior.as_ref().expect("serialization must be enabled in order to serialize on close");
				let path=self.buffer.take().unwrap().into_path();

				self.len=0;
				let (path,store)=if let Some(store)=&store{
					(path,store)
				}else{
					let temp=generate_path();
					fs::rename(&path,&temp).unwrap();

					(temp,&path)
				};

				let mut storefile=BufWriter::new(OpenOptions::new().create(true).write(true).open(&store).unwrap());
				let vecfile=unsafe{Self::open(&path)}.unwrap();

				for v in vecfile.iter(){behavior.save(v,&mut storefile).unwrap()}

				mem::drop(vecfile);
				fs::remove_file(path).unwrap();
			},
			#[allow(unreachable_patterns)]
			x=>panic!("close behavior {:?} is not available with this feature set",x)
		}
	}
	/// reference the close behavior
	pub fn close_behavior(&self)->&OnClose{&self.closebehavior}
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
	#[cfg(any(feature="serial-json",feature="serial-rmp"))]
	/// enable serialization and set the serial behavior depending on the feature. if multiple serial features are enabled, use serialize_on_close for finer control over exactly what serial behavior to use. Note that this does not set the close behavior, but serialize_on_close does
	pub fn enable_serialization(&mut self) where E:DeserializeOwned+Serialize{
		#[cfg(feature="serial-json")]
		{self.serialbehavior=Some(Arc::new(SerialJson::new(true)))}
		#[cfg(feature="serial-rmp")]
		{self.serialbehavior=Some(Arc::new(SerialRMP::new()))}
	}
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
	/// get the current close behavior
	pub fn get_close_behavior(&self)->OnClose{self.closebehavior.clone()}
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
	/// groups every N items into chunks, dropping the remainder. if the close behavior is serialize, note that this does not preserve serial behavior
	pub fn into_chunks<const N:usize>(mut self)->FileVec<[E;N]>{
		self.truncate(self.len-self.len%N);

		//if mem::size_of::<E>()==0{return FileVec::zst(self.len,self.path())}
		if self.buffer.is_none(){return FileVec::new()}

		let closebehavior=mem::replace(&mut self.closebehavior,OnClose::Persist);
		let path=self.buffer.take().unwrap().into_path();

		self.close();
		let mut result=unsafe{FileVec::open(path).unwrap()};

		/*#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]{
			result.serialbehavior=self.serialbehavior.take().map(|b|unsafe{
				let b:ArraySB<Arc<dyn SerialBehavior<E,BufReader<File>,BufWriter<File>>>>=ArraySB(b);
				let a:Arc<dyn SerialBehavior<[E;N],BufReader<File>,BufWriter<File>>>=Arc::new(b);
				// serial behavior is only some if static
				let p:*const  dyn         SerialBehavior<[E;N],BufReader<File>,BufWriter<File>> =Arc::into_raw(a);
				let p:*const (dyn 'static+SerialBehavior<[E;N],BufReader<File>,BufWriter<File>>)=mem::transmute_copy(&p);

				Arc::from_raw(p)
			});
		}*/

		result.closebehavior=closebehavior;
		result
	}
	/// checks if empty
	pub fn is_empty(&self)->bool{self.len==0}
	/// set the len to 0 and reset the path without calling drop on the components.
	pub fn leak(&mut self){
		self.buffer=None;
		self.len=0;
	}
	/// returns the length of the file vec
	pub fn len(&self)->usize{self.len}
	#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
	/// use the enabled serialization to load from a file
	pub fn load<P:AsRef<Path>>(&mut self,path:P)->IOResult<()> where E:Debug{
		use std::io::{BufRead,ErrorKind as IOErrorKind};
		let serialbehavior=if let Some(s)=self.serialbehavior.clone(){s}else{return Err(IOError::other("serial behavior not set"))};

		let file=OpenOptions::new().read(true).open(path).unwrap();
		let mut reader=BufReader::new(file);

		Ok(while reader.buffer().len()>0||reader.fill_buf()?.len()>0{
			let b=serialbehavior.load(&mut reader);
			if let Err(b)=&b&&b.kind()==IOErrorKind::UnexpectedEof{		// hack to make json thing work
				if reader.buffer().len()==0&&reader.fill_buf()?.len()==0{break}
			}

			self.push(b?);
		})
	}
	/// allow len adjustment from other files in the crate
	pub (crate) unsafe fn len_mut(&mut self)->&mut usize{&mut self.len}
	/// creates new file vec. The file won't be created and the vec won't allocate until items are added to it
	pub fn new()->Self{
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0);

		Self{
			buffer:None,
			len:0,
			closebehavior:OnClose::Delete,
			#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
			serialbehavior:None
		}
	}
	/// opens a file as a FileVec. The bytes in the file must be valid when interpreted as [T], the file should not be modified while in use, and the file must not be open in any other file vec or other memory mapping. An empty file will be created when the file vec allocates if the file doesn't exist. Regardless of whether the file already existed, the FileVec's persistence flag will be initialized to true
	pub unsafe fn open<P:AsRef<Path>>(path:P)->IOResult<Self>{
		assert_eq!(page_size::get()%mem::align_of::<E>(),0);
		assert_ne!(mem::size_of::<E>(),0);

		let buffer=Buffer::open(path.as_ref().into())?;
		let closebehavior=OnClose::Persist;

		let len=buffer.as_slice().len();
		let buffer=Some(buffer);

		Ok(Self{
			buffer,len,closebehavior,
			#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
			serialbehavior:None
		})
	}
	/// references the file path. returns none if the vec is new and empty and hasn't created a file yet
	pub fn path(&self)->Option<&Path>{
		if let Some(b)=&self.buffer{Some(&b.path)}else{None}
	}
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
		let newcapacity=if required<=self.capacity(){return}else{required.next_power_of_two()};

		if   self.buffer.is_some(){self.buffer=self.buffer.take().unwrap().with_size(newcapacity).unwrap().into()}
		else{self.buffer=Buffer::new(None,newcapacity).unwrap().into()}
	}
	/// reserves capacity for at least additional more items in the backing file
	pub fn reserve_exact(&mut self,additional:usize){
		let required=additional.saturating_add(self.len());
		let newcapacity=if required<=self.capacity(){return}else{required};

		if   self.buffer.is_some(){self.buffer=self.buffer.take().unwrap().with_size(newcapacity).unwrap().into()}
		else{self.buffer=Buffer::new(None,newcapacity).unwrap().into()}
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
	#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
	/// enable serialization, set the close behavior to serialize and set the serial behavior
	pub fn serialize_on_close<B:'static+SerialBehavior<E,BufReader<File>,BufWriter<File>>>(&mut self,behavior:B){
		self.serialbehavior=Some(Arc::new(behavior));
		self.set_close_behavior(OnClose::Serialize(None));
	}
	/// set the close behavior
	pub fn set_close_behavior(&mut self,closebehavior:OnClose){self.closebehavior=closebehavior}
	/// if persistent, set the close behavior to persist if it is delete. if not persistent, set the close behavior to delete
	pub fn set_persistent(&mut self,persistent:bool){
		if persistent{
			if let OnClose::Delete=&self.closebehavior{self.closebehavior=OnClose::Persist}
		}else{
			self.closebehavior=OnClose::Delete
		}
	}
	/// sets the length of the file vec. The data must be initialized up to the new length, and the new length must be within capacity
	pub unsafe fn set_len(&mut self,len:usize){self.len=len}
	/// sets the path to store the data in. If data is already in the file, the file will be renamed. The path should not be modified while in use, and the path must not be open in any other file vec or other memory mapping. Caution: it is possible in err case for  the remapping after the attempt to have also failed, causing a memory leak.
	pub unsafe fn set_path<P:AsRef<Path>>(&mut self,path:P)->IOResult<()>{
		if self.buffer.is_none(){
			self.buffer=Some(Buffer::new(path.as_ref().to_path_buf(),1)?);
			Ok(())
		}else{
			let oldpath:PathBuf=self.path().unwrap().into();
			self.buffer=unsafe{Some(self.buffer.take().unwrap().with_path(path.as_ref().into()).inspect_err(|_|self.buffer=Buffer::open(oldpath).ok())?)};
			Ok(())
		}
	}
	/// shrinks the capacity of the vector with a lower bound
	pub fn shrink_to(&mut self,mut mincap:usize){
		if mincap>=self.capacity(){return}
		if mincap<self.len{mincap=self.len}

		self.buffer=Some(self.buffer.take().unwrap().with_size(mincap).unwrap());
	}
	/// Shrinks the capacity of the vector as much as possible
	pub fn shrink_to_fit(&mut self){self.shrink_to(self.len)}
	/// Returns the remaining spare capacity of the vector as a slice of MaybeUninit<E>
	pub fn spare_capacity_mut(&mut self)->&mut [MaybeUninit<E>]{self.split_at_spare_mut().1}
	/// Returns vector content as a slice of T, along with the remaining spare capacity of the vector as a slice of MaybeUninit<T>.
	pub fn split_at_spare_mut(&mut self)->(&mut [E],&mut [MaybeUninit<E>]){
		let items1=self.buffer.as_mut_slice();
		unsafe{								// the items up to len are initialized
			mem::transmute(items1.split_at_mut(self.len))
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
pub struct FileVec<E>{
	buffer:Option<Buffer<E>>,
	len:usize,
	closebehavior:OnClose,
	#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
	serialbehavior:Option<Arc<dyn SerialBehavior<E,BufReader<File>,BufWriter<File>>>>
}
#[derive(Debug)]
/// internal buffer management
struct Buffer<E>{map:MmapMut,path:PathBuf,phantom:PhantomData<E>}
#[derive(Clone,Debug,Default)]
/// Enumerate the file vec close behavior. When closed or dropped...
pub enum OnClose{
	/// leave the backing file as-is on the disk (not recommended for non pod types)
	Persist,
	#[default]
	/// delete the backing file (default behavior)
	Delete,
	/// serialize the data for safe storage of non pod types, optionally with a separate path for the storage. Requires the file vec have serialization enabled, such as through filevec.enable_serialization(), which in turn requires a serial feature
	Serialize(Option<PathBuf>)
}
#[cfg_attr(feature="serial-json",derive(Clone,Copy,Debug,Default))]
#[cfg(feature="serial-json")]
/// serialize data with serde-json
pub struct SerialJson{pretty:bool}
#[cfg_attr(feature="serial-rmp",derive(Clone,Copy,Debug,Default))]
#[cfg(feature="serial-rmp")]
/// serialize data with rmp-serde
pub struct SerialRMP;

#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
mod serial_adapters{
	#[cfg(feature="serial-json")]
	impl SerialJson{
		/// create a new serialization behavior that uses serde-json
		pub fn new(pretty:bool)->Self{
			Self{pretty}
		}
	}
	#[cfg(feature="serial-rmp")]
	impl SerialRMP{
		/// create a new serialization behavior that uses rmp-serde
		pub fn new()->Self{Self}
	}
	#[cfg(feature="serial-json")]
	impl<E:DeserializeOwned+Serialize,R:Read,W:Write> SerialBehavior<E,R,W> for SerialJson{
		fn load(&self        ,reader:&mut R)->IOResult<E >{
			use std::io::ErrorKind as IOErrorKind;
			//Ok(json_decode::from_reader(&mut *reader)?)

			let mut e=serde_json::Deserializer::from_reader(reader).into_iter::<E>();
			e.next().ok_or_else(||IOError::new(IOErrorKind::UnexpectedEof,"stream should have data at this point but no items are found")).and_then(|e|Ok(e?))
		}
		fn save(&self,data:&E,writer:&mut W)->IOResult<()>{
			if self.pretty{json_encode::to_writer_pretty(&mut *writer,data)?}else{json_encode::to_writer(&mut *writer,data)?}
			writeln!(writer)
		}
	}
	#[cfg(feature="serial-rmp")]
	impl<E:DeserializeOwned+Serialize,R:Read,W:Write> SerialBehavior<E,R,W> for SerialRMP{
		fn load(&self         ,reader:&mut R)->IOResult<E >{Ok(rmp_decode::from_read(reader       ).map_err(IOError::other)?)}
		fn save(&self,data:& E,writer:&mut W)->IOResult<()>{Ok(rmp_encode::write(&mut *writer,data).map_err(IOError::other)?)}
	}
	impl<E,R,W> SerialBehavior<E,R,W> for Arc<dyn SerialBehavior<E,R,W>>{
		fn load(&self        ,reader:&mut R)->IOResult< E>{(**self).load(     reader)}
		fn save(&self,data:&E,writer:&mut W)->IOResult<()>{(**self).save(data,writer)}
	}
	/// serialization behavior to apply when enabling serialization
	pub trait SerialBehavior<E,R,W>:Debug+Send+Sync{
		/// deserialization behavior to apply when loading
		fn load(&self        ,reader:&mut R)->IOResult<E >;
		/// serialization behavior to apply when saving
		fn save(&self,data:&E,writer:&mut W)->IOResult<()>;
	}
	use super::*;
}
#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
pub use serial_adapters::SerialBehavior;

use crate::{
	FinalizeDrop,iter::{Drain,ExtractIf}
};
use memmap2::MmapMut;
#[cfg(feature="serial-json")]
use serde_json::ser as json_encode;
#[cfg(feature="serial-rmp")]
use rmp_serde::{decode as rmp_decode,encode as rmp_encode};
#[cfg(any(feature="serial-json",feature="serial-rmp"))]
use serde::{Serialize,de::DeserializeOwned};
#[cfg(any(feature="serial-json",feature="serial-rmp"))]
use std::io::{Read,Write};
#[cfg(any(feature="serial-custom",feature="serial-json",feature="serial-rmp"))]
use std::{
	fs::File,io::{BufReader,BufWriter,Error as IOError},sync::Arc
};
use std::{
	borrow::{Borrow,BorrowMut},cell::Cell,cmp::PartialEq,fmt::Debug,fs::{OpenOptions,self},io::{Result as IOResult},iter::FromIterator,marker::PhantomData,mem::{MaybeUninit,self},ops::{Bound,Deref,DerefMut,RangeBounds},path::{PathBuf,Path},ptr
};
