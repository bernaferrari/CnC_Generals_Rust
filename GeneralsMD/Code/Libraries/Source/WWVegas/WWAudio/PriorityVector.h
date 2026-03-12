#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __PRIORITY_VECTOR_H
#define __PRIORITY_VECTOR_H

#include "Vector.H"

////////////////////////////////////////////////////////////////////
//
//	PriorityVectorClass
//
////////////////////////////////////////////////////////////////////
template<class T>
class PriorityVectorClass : public DynamicVectorClass<T>
{
	public:

		virtual bool	Process_Head (T &object);
		virtual bool	Add_Low (T const &object);
		virtual bool	Add_High (T const &object);

		/*PriorityVectorClass<T> & operator= (PriorityVectorClass<T> const & rvalue) {
			DynamicVectorClass<T>::operator= (rvalue);
			return(*t8his);
		}*/
};


////////////////////////////////////////////////////////////////////
//
//	Process_Head
//
////////////////////////////////////////////////////////////////////
template <class T>
__inline bool PriorityVectorClass<T>::Process_Head (T &object)
{
	bool retval = false;	
	if (Vector != NULL) {
		
		// Pass the object back to the caller
		object = Vector[0];

		//
		//	Move the head object to the end of the list
		//
		for (int index = 1; index < ActiveCount; index ++) {
			Vector[index - 1] = Vector[index];
		}
		Vector[ActiveCount - 1] = object;

		// Success!
		retval = true;
	}

	return retval;
}


////////////////////////////////////////////////////////////////////
//
//	Add_Low 
//
////////////////////////////////////////////////////////////////////
template <class T>
__inline bool PriorityVectorClass<T>::Add_Low (T const &object)
{
	return DynamicVectorClass<T>::Add (object);
}


////////////////////////////////////////////////////////////////////
//
//	Add_High
//
////////////////////////////////////////////////////////////////////
template <class T>
__inline bool PriorityVectorClass<T>::Add_High (T const &object)
{
	return DynamicVectorClass<T>::Add_Head (object);
}

#endif //__PRIORITY_VECTOR_H

