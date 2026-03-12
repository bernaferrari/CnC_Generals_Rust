/******************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     Base class for reference counted classes.
*     Use with the reference counting smart pointer RefPtr<Type>
*
*     Release() is virtual. This helps support cached object and singletons
*
* PROGRAMMER
*     Steven Clinard
*     $Author:  $
*
* VERSION INFO
*     $Modtime:  $
*     $Revision:  $
*
******************************************************************************/

#ifndef REFCOUNTED_H
#define REFCOUNTED_H

#include <assert.h>

class RefCounted
	{
	protected:
		RefCounted()
			: mRefCount(0)
			{}

		RefCounted(const RefCounted&)
			: mRefCount(0)
			{}

		inline const RefCounted& operator=(const RefCounted&)
			{}

		virtual ~RefCounted()
			{assert(mRefCount == 0);}

		// Should not be allowed by default
		inline virtual bool operator==(const RefCounted&) const
			{return false;}

		inline bool operator!=(const RefCounted&) const
			{return false;}

		// Add reference
		inline void AddReference(void)
			{++mRefCount;}

		// Release reference
		inline virtual void Release(void)
			{if (--mRefCount == 0) delete this;}

		inline int ReferenceCount(void) const
			{return mRefCount;}

	private:
		friend class RefPtrBase;

		unsigned int mRefCount;
	};

#endif // REFCOUNTED_H
