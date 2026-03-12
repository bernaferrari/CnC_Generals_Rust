#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#if !defined(TRUE_FALSE_DEFINED) && !defined(__BORLANDC__) && (_MSC_VER < 1100) && !defined(__WATCOMC__)
#define TRUE_FALSE_DEFINED

/**********************************************************************
**      The "bool" integral type was defined by the C++ comittee in
**      November of '94. Until the compiler supports this, use the following
**      definition.
*/
#ifdef _MSC_VER

#include        "yvals.h"
#define bool    unsigned

#elif defined(_UNIX)

/////#define bool    unsigned

#else

enum {false=0,true=1};
typedef int bool;

#endif

#endif
