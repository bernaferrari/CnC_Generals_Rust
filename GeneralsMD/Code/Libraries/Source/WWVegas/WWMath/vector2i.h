#if defined(_MSC_VER)
#pragma once
#endif

#ifndef VECTOR2I_H
#define VECTOR2I_H

#include "always.h"

class Vector2i
{
public:

	int		I;
	int		J;

	WWINLINE Vector2i(void);
	WWINLINE Vector2i(int i,int j);
	
	WWINLINE void Set(int i, int j);

	WWINLINE void Swap(Vector2i & other);

	WWINLINE bool			operator== (const Vector2i & v) const;
	WWINLINE bool			operator!= (const Vector2i& v) const;
	WWINLINE const	int&	operator[] (int n) const;
	WWINLINE int&			operator[] (int n);
};


WWINLINE Vector2i::Vector2i(void)
{
}

WWINLINE Vector2i::Vector2i(int i,int j) 
{ 
	I = i; J = j; 
}

WWINLINE bool Vector2i::operator == (const Vector2i & v) const
{ 
	return (I == v.I && J == v.J );	
}

WWINLINE bool Vector2i::operator !=	(const Vector2i& v) const
{ 
	return !(I == v.I && J == v.J);	
}

WWINLINE const int& Vector2i::operator[] (int n) const				
{ 
	return ((int*)this)[n]; 
}

WWINLINE int& Vector2i::operator[] (int n)
{ 
	return ((int*)this)[n]; 
}

WWINLINE void Vector2i::Set(int i, int j) { I = i; J = j; }

WWINLINE void Vector2i::Swap(Vector2i & other)
{
	// this could use MMX..

	I ^= other.I;
	other.I ^= I;
	I ^= other.I;

	J ^= other.J;
	other.J ^= J;
	J ^= other.J;
}


#endif