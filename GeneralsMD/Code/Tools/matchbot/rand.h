#ifndef __RAND_H__
#define __RAND_H__

#include <cstdlib>

class RandClass
{
public:
	RandClass(int start = 0);
	~RandClass()
	{}


	int Int(void);
	double Double(void);
	int Int(int low, int high);
	double Double(double low, double high);

private:

	unsigned int randomValue( void );
	unsigned int seed[6];

};

#endif /* __RAND_H__ */

