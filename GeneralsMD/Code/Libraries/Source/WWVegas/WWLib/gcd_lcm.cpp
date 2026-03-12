#include "gcd_lcm.h"

unsigned int Greatest_Common_Divisor(unsigned int a, unsigned int b)
{
	// This uses Euclid's algorithm
	if (b == 0) {
		return a;
	} else {
		return Greatest_Common_Divisor(b, a % b);
	}
}


unsigned int Least_Common_Multiple(unsigned int a, unsigned int b)
{
	return (a * b) / Greatest_Common_Divisor(a, b);
}
