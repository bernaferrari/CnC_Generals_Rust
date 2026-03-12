#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef STL_H
#define STL_H

/*
**	This header file includes the Standard Template Library Headers
**	and disables certian warnings
*/

#if (_MSC_VER >= 1200)
#pragma warning(push,3)
#endif

#include <map>

#if (_MSC_VER >= 1200)
#pragma warning(pop)
#endif


#endif
