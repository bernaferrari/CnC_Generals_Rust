#include "wwmath.h"
#include <stdlib.h>

/*
**
*/
float		WWMath::Random_Float(void) 
{ 
	return ((float)(rand() & 0xFFF)) / (float)(0xFFF); 
}
