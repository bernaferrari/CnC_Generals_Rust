#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef BOUND_H
#define BOUND_H

template<class T> inline
T Bound(T original, T minval, T maxval)
{
	if (original < minval) return(minval);
	if (original > maxval) return(maxval);
	return(original);
};
#if defined(__WATCOMC__)
//int Bound(int, int, int);
//signed int Bound(signed int, signed int, signed int);
//unsigned Bound(unsigned, unsigned, unsigned);
//long Bound(long, long, long);
//float Bound(float, float, float);
//double Bound(double, double, double);
#endif


#endif

