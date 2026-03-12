#if defined(_MSC_VER)
#pragma once
#endif


#ifndef POINTERREMAP_H
#define POINTERREMAP_H

#ifdef _UNIX
#include "osdep/osdep.h"
#endif

#include "always.h"
#include "vector.h"

class RefCountClass;


class PointerRemapClass 
{
	public:
		
		PointerRemapClass(void);
		~PointerRemapClass(void);

		void		Reset(void);
		void		Process(void);

		void		Register_Pointer (void *old_pointer, void *new_pointer);

#ifdef WWDEBUG
		void		Request_Pointer_Remap (void **pointer_to_convert,const char * file,int line);
		void		Request_Ref_Counted_Pointer_Remap (RefCountClass **pointer_to_convert,const char * file, int line);
#else
		void		Request_Pointer_Remap (void **pointer_to_convert);
		void		Request_Ref_Counted_Pointer_Remap (RefCountClass **pointer_to_convert);
#endif

	private:

		struct PtrPairStruct
		{
			PtrPairStruct(void) {}
			PtrPairStruct(void * oldptr,void * newptr) : OldPointer(oldptr),NewPointer(newptr) {}
			bool operator == (const PtrPairStruct & that) { return ((OldPointer == that.OldPointer) && (NewPointer == that.NewPointer)); } 
			bool operator != (const PtrPairStruct & that) { return !(*this == that); } 
			
			void *		OldPointer;
			void *		NewPointer;
		};
		
		struct PtrRemapStruct
		{
			PtrRemapStruct(void) {}
			bool operator == (const PtrRemapStruct & that) { return (PointerToRemap == that.PointerToRemap); } 
			bool operator != (const PtrRemapStruct & that) { return !(*this == that); } 
			
			void **			PointerToRemap;
#ifdef WWDEBUG
			const char *	File;
			int				Line;
#endif
		};

		void		Process_Request_Table(DynamicVectorClass<PtrRemapStruct> & request_table,bool refcount);
		static int __cdecl ptr_pair_compare_function(void const * ptr1, void const * ptr2);
		static int __cdecl ptr_request_compare_function(void const * ptr1, void const * ptr2);

		/*
		**	Array of pointers associated with ID values to assist in swizzling.
		*/
		DynamicVectorClass<PtrPairStruct>	PointerPairTable;
		DynamicVectorClass<PtrRemapStruct>	PointerRequestTable;
		DynamicVectorClass<PtrRemapStruct>	RefCountRequestTable;
};


#endif
