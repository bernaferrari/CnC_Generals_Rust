#if defined(_MSC_VER)
#pragma once
#endif

#ifndef VECTOR3I_H
#define VECTOR3I_H

#include "always.h"

class Vector3i
{
public:

	int		I;
	int		J;
	int		K;

	WWINLINE Vector3i(void);
	WWINLINE Vector3i(int i,int j,int k);

	WWINLINE bool			operator== (const Vector3i & v) const;
   WWINLINE bool			operator!= (const Vector3i& v) const;
	WWINLINE const	int&	operator[] (int n) const;
	WWINLINE int&			operator[] (int n);
};


WWINLINE Vector3i::Vector3i(void)
{
}

WWINLINE Vector3i::Vector3i(int i,int j,int k) 
{ 
	I = i; J = j; K = k; 
}

WWINLINE bool Vector3i::operator == (const Vector3i & v) const
{ 
	return (I == v.I && J == v.J && K == v.K);	
}

WWINLINE bool Vector3i::operator !=	(const Vector3i& v) const
{ 
	return !(I == v.I && J == v.J && K == v.K);	
}

WWINLINE const int& Vector3i::operator[] (int n) const				
{ 
	return ((int*)this)[n]; 
}

WWINLINE int& Vector3i::operator[] (int n)
{ 
	return ((int*)this)[n]; 
}

#endif