#ifndef __UTILS_H
#define __UTILS_H

#pragma warning (push, 3)
#include "Mss.H"
#pragma warning (pop)

/////////////////////////////////////////////////////////////////////////////
//
// Macros
//
#define SAFE_DELETE(pobject)					\
			if (pobject) {							\
				delete pobject;					\
				pobject = NULL;					\
			}											\

#define SAFE_DELETE_ARRAY(pobject)			\
			if (pobject) {							\
				delete [] pobject;				\
				pobject = NULL;					\
			}											\

#define SAFE_FREE(pobject)						\
			if (pobject) {							\
				::free (pobject);					\
				pobject = NULL;					\
			}											\


/////////////////////////////////////////////////////////////////////////////
//
//	MMSLockClass
//
/////////////////////////////////////////////////////////////////////////////
class MMSLockClass
{
	public:
		MMSLockClass (void) { ::AIL_lock (); }
		~MMSLockClass (void) { ::AIL_unlock (); }


	static CRITICAL_SECTION _MSSLockCriticalSection;
};


////////////////////////////////////////////////////////////////////////////
//
//  Get_Filename_From_Path
//
__inline LPCTSTR
Get_Filename_From_Path (LPCTSTR path)
{
	// Find the last occurance of the directory deliminator
	LPCTSTR filename = ::strrchr (path, '\\');
	if (filename != NULL) {
		// Increment past the directory deliminator
		filename ++;
	} else {
		filename = path;
	}

	// Return the filename part of the path
	return filename;
}


#endif //__UTILS_H
