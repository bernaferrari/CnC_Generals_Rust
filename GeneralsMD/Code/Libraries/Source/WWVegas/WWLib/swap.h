#ifndef SWAP_H
#define SWAP_H

/**********************************************************************
**	Swaps two objects.
*/
template<class T>
void swap(T & left, T & right)
{
	T temp;
	temp = left;
	left = right;
	right = temp;
}

#endif
