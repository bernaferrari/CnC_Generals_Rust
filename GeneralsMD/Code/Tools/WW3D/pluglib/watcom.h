#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#if !defined(WATCOM_H) && defined(__WATCOMC__)
#define WATCOM_H


/**********************************************************************
**	The "bool" integral type was defined by the C++ comittee in
**	November of '94. Until the compiler supports this, use the following
**	definition.
*/
#include	"bool.h"

// Turn all warnings into errors.
#pragma warning * 0

// Disables warning when "sizeof" is used on an object with virtual functions.
#pragma warning 549 9

// Disable the "Integral value may be truncated during assignment or initialization".
#pragma warning 389 9

// Allow constructing a temporary to be used as a parameter.
#pragma warning 604 9

// Disable the construct resolved as an expression warning.
#pragma warning 595 9

// Disable the strange "construct resolved as a declaration/type" warning.
#pragma warning 594 9

// Disable the "pre-compiled header file cannot be used" warning.
#pragma warning 698 9

// Disable the "temporary object used to initialize a non-constant reference" warning.
#pragma warning 665 9

// Disable the "pointer or reference truncated by cast. Cast is supposed to REMOVE warnings, not create them.
#pragma warning 579 9

// Disable the warning that suggests a null destructor be placed in class definition.
#pragma warning 656 9

// Disable the warning about moving empty constructors/destructors to the class declaration.
#pragma warning 657 9

// No virtual destructor is not an error in C&C.
#pragma warning 004 9

// Integral constant will be truncated warning is usually ok when dealing with bitfields.
#pragma warning 388 9

// Turns off unreferenced function parameter warning.
//#pragma off(unreferenced)

/*
**	The "bool" integral type was defined by the C++ comittee in
**	November of '94. Until the compiler supports this, use the following
**	definition.
*/
#include	"bool.h"

#if !defined(__BORLANDC__)
#define M_E         2.71828182845904523536
#define M_LOG2E     1.44269504088896340736
#define M_LOG10E    0.434294481903251827651
#define M_LN2       0.693147180559945309417
#define M_LN10      2.30258509299404568402
#define M_PI        3.14159265358979323846
#define M_PI_2      1.57079632679489661923
#define M_PI_4      0.785398163397448309616
#define M_1_PI      0.318309886183790671538
#define M_2_PI      0.636619772367581343076
#define M_1_SQRTPI  0.564189583547756286948
#define M_2_SQRTPI  1.12837916709551257390
#define M_SQRT2     1.41421356237309504880
#define M_SQRT_2    0.707106781186547524401
#endif


#endif
