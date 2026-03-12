#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef GCD_LCM_H

#include "always.h"

// The greatest common divisor (GCD) is the greatest integer which is a
// divisor of given positive integers.
unsigned int Greatest_Common_Divisor(unsigned int a, unsigned int b);

// The least common multiple (LCM) is the least integer of which given
// positive integers are divisors.
unsigned int Least_Common_Multiple(unsigned int a, unsigned int b);

#endif