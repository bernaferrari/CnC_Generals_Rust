#ifndef SMARTPTR_H
#define SMARTPTR_H

#include	"noinit.h"

template<class T>
class SmartPtr
{
	public:
		SmartPtr(NoInitClass const &) {}
		SmartPtr(T * realptr = 0) : Pointer(realptr) {}
		SmartPtr(SmartPtr const & rvalue) : Pointer(rvalue.Pointer) {}
		~SmartPtr(void) {Pointer = 0;}

		operator T * (void) const {return(Pointer);}

		operator long (void) const {return((long)Pointer);}

		bool Is_Valid(void) const {return(Pointer != 0);}

//		SmartPtr<T> operator ++ (int) {assert(Pointer != 0);SmartPtr<T> temp = *this;++Pointer;return(temp);}
//		SmartPtr<T> & operator ++ (void) {assert(Pointer != 0);++Pointer;return(*this);}
//		SmartPtr<T> operator -- (int) {assert(Pointer != 0);SmartPtr<T> temp = *this;--Pointer;return(temp);}
//		SmartPtr<T> & operator -- (void) {assert(Pointer != 0);--Pointer;return(*this);}

		SmartPtr & operator = (SmartPtr const & rvalue) {Pointer = rvalue.Pointer;return(*this);}
		T * operator -> (void) const {/*assert(Pointer != 0);*/return(Pointer);}
		T & operator * (void) const {/*assert(Pointer != 0);*/return(*Pointer);}

	private:
		T * Pointer;
};


#endif
