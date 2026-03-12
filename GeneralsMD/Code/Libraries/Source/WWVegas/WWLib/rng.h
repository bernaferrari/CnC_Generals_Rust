#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef RNG_H
#define RNG_H

/*
**	This is an abstract interface class for a random number generator. It serves only to
**	provide random numbers.
*/
class RandomNumberGenerator {
	public:
		virtual ~RandomNumberGenerator() {}

		virtual void Get_Block(void * output, unsigned int size) = 0;
};


#endif

