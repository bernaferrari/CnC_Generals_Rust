#ifndef SRANDOM_H
#define SRANDOM_H

#include "random.h"	// for the helper RNG

//
//	SecureRandomClass - Generate random values that are suitable for use in cryptographic
//		applications (under UNIX at least).
//
//	NOTE: The seed generation under windows isn't nearly as strong as under UNIX due to
//		the lack of /dev/random.  If you have a good random source you can call 'Add_Seeds'
//		to improve the quality of the seed value.
//
// Currently the random seed generation under windows is piss-poor so make sure and call
//		Add_Seeds with some interesting data if you're going to use this!
//
//	The seed values should be unguessable by an outside viewer and the random values have
//		good distribution properties.
//
class SecureRandomClass
{
public:
		SecureRandomClass();
		~SecureRandomClass();

		unsigned long				Randval();				// get a 32 bit random value

																	// Add randomness to the seed pool
		void							Add_Seeds(unsigned char *values, int length);

private:
		void							Generate_Seed(void);	// Generate the inital seed

		enum
		{
			SeedLength =			16,		// bytes in random seed
			SHADigestBytes =		20			// length of SHA hash
		};

		static bool					Initialized;			// has the seed been initialized?
		static unsigned char		Seeds[SeedLength];	// random seed values
		static unsigned int		RandomCache[SHADigestBytes / sizeof(unsigned int)];
		static int					RandomCacheEntries;
		static unsigned int		Counter;

		static Random3Class		RandomHelper;
};

#endif