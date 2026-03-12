/*************************************************************************** 
 *																									* 
 *						 Project	Name : G														* 
 *																									* 
 *							 File	Name : SLNODE.H									 		* 
 *																									* 
 *							Programmer : Philip W. Gorrow									* 
 *																									* 
 *							Start	Date : 03/11/97											* 
 *																									* 
 *						  Last Update : March 11, 1997 [PWG]							* 
 *																									* 
 *-------------------------------------------------------------------------* 
 *	Functions:																					* 
 *	- - -	- - -	- - -	- - -	- - -	- - -	- - -	- - -	- - -	- - -	- - -	- - -	*/
#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef __SLNODE_H__
#define __SLNODE_H__

#ifndef ALWAYS_H
#include "always.h"
#endif

#include "mempool.h"

#ifndef NULL
#define NULL 0
#endif

//	Forward references for friend	classes
template	<class T> class SList;

//
//	The node	class	is	responsible	for maintaining the links between
//	data in a linked list.	It	works	with the	Single List	Class	to
//	manage a	singularly linked	list of objects.	
//

class	GenericSLNode : public AutoPoolClass<GenericSLNode, 256>
{
	protected:
		void* Internal_Get_Next(void) { return NodeNext; };
		void Internal_Set_Next(void* n) { NodeNext=n; };
		void *Internal_Get_Data(void) { return NodeData; };
		void Internal_Get_Data(void* d) { NodeData=d; };

		//
		//	Note that their is only one constructor for this class and it
		//	requires you to provide an object.  Furthermore it can be
		// created from anything but a friend or parent class.
		//
		GenericSLNode(void *obj)
			{NodeData = obj; NodeNext = 0; };

		//
		//	You cannot declare a node class without giving it a data object.
		//	Defining this type of constructor as private to the class insures
		//	that it cannot be used.
		//
	private:
		GenericSLNode(void) {};

		void		*NodeNext;			//	Next Node in the list chain
		void		*NodeData;			//	Current Node in the list chain
};


template	<class T>
class	SLNode : public GenericSLNode
{
	public:
		//
		//	Since	the list	class	manages the	Node Class it must be able	to
		//	access its private data.
		//
		friend class SList<T>;

		SLNode<T>* Next(void) { return reinterpret_cast<SLNode<T>*>(Internal_Get_Next()); }
		T *Data(void) { return reinterpret_cast<T*>(Internal_Get_Data()); }

		void Set_Next(SLNode<T>* n) { Internal_Set_Next(reinterpret_cast<void*>(n)); }

	protected:
		//
		//	Note that their is only one constructor for this class and it
		//	requires you to provide an object.  Furthermore it can be
		// created from anything but a friend or parent class.
		//
		SLNode(T *obj) : GenericSLNode(obj) {}

		//
		//	You cannot declare a node class without giving it a data object.
		//	Defining this type of constructor as private to the class insures
		//	that it cannot be used.
		//
	private:
		SLNode(void) {};
};

#endif
