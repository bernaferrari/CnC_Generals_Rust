#include "matrix4.h"
#include <assert.h>

/***********************************************************************************************
 * Matrix4::Multiply -- Multiply two Matrix4's together                                        *
 *                                                                                             *
 * INPUT:                                                                                      *
 *  a - first operand                                                                          *
 *  b - second operand                                                                         *
 *  res - pointer to matrix to store the result in (must not point to a or b)                  *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/13/99   gth : Created.                                                                 *
 *=============================================================================================*/
void Matrix4::Multiply(const Matrix4 &a,const Matrix4 &b,Matrix4 * res)
{
	assert(res != &a);
	assert(res != &b);

	#define ROWCOL(i,j) a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j] + a[i][3]*b[3][j]

	(*res)[0][0] = ROWCOL(0,0);
	(*res)[0][1] = ROWCOL(0,1);
	(*res)[0][2] = ROWCOL(0,2);
	(*res)[0][3] = ROWCOL(0,3);

	(*res)[1][0] = ROWCOL(1,0);
	(*res)[1][1] = ROWCOL(1,1);
	(*res)[1][2] = ROWCOL(1,2);
	(*res)[1][3] = ROWCOL(1,3);

	(*res)[2][0] = ROWCOL(2,0);
	(*res)[2][1] = ROWCOL(2,1);
	(*res)[2][2] = ROWCOL(2,2);
	(*res)[2][3] = ROWCOL(2,3);

	(*res)[3][0] = ROWCOL(3,0);
	(*res)[3][1] = ROWCOL(3,1);
	(*res)[3][2] = ROWCOL(3,2);
	(*res)[3][3] = ROWCOL(3,3);

	#undef ROWCOL
}


/***********************************************************************************************
 * Matrix4::Multiply -- Multiply a Matrix3D * Matrix4                                          *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/13/99   gth : Created.                                                                 *
 *=============================================================================================*/
void Matrix4::Multiply(const Matrix3D &a,const Matrix4 &b,Matrix4 * res)
{
	assert(res != &b);

	#define ROWCOL(i,j) a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j] + a[i][3]*b[3][j]

	(*res)[0][0] = ROWCOL(0,0);
	(*res)[0][1] = ROWCOL(0,1);
	(*res)[0][2] = ROWCOL(0,2);
	(*res)[0][3] = ROWCOL(0,3);

	(*res)[1][0] = ROWCOL(1,0);
	(*res)[1][1] = ROWCOL(1,1);
	(*res)[1][2] = ROWCOL(1,2);
	(*res)[1][3] = ROWCOL(1,3);

	(*res)[2][0] = ROWCOL(2,0);
	(*res)[2][1] = ROWCOL(2,1);
	(*res)[2][2] = ROWCOL(2,2);
	(*res)[2][3] = ROWCOL(2,3);

	(*res)[3][0] = b[3][0];				// last row of a is 0,0,0,1
	(*res)[3][1] = b[3][1];				// this leaves the last row of b unchanged
	(*res)[3][2] = b[3][2];
	(*res)[3][3] = b[3][3];

	#undef ROWCOL
}


/***********************************************************************************************
 * Matrix4::Multiply -- Multiply a Matrix4 * Matrix3D                                          *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *=============================================================================================*/
void Matrix4::Multiply(const Matrix4 & a,const Matrix3D & b,Matrix4 * res)
{
	assert(res != &a);
	
	// ROWCOL multiplies a row of 'a' by one of the first three columns of 'b' (4th entry in b is zero)
	// ROWCOL4 multiplies a row of 'a' by the fourth column of 'b' (4th entry in b is one)

	#define ROWCOL(i,j) a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j]
	#define ROWCOL4(i,j) a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j] + a[i][3]

	(*res)[0][0] = ROWCOL(0,0);
	(*res)[0][1] = ROWCOL(0,1);
	(*res)[0][2] = ROWCOL(0,2);
	(*res)[0][3] = ROWCOL4(0,3);

	(*res)[1][0] = ROWCOL(1,0);
	(*res)[1][1] = ROWCOL(1,1);
	(*res)[1][2] = ROWCOL(1,2);
	(*res)[1][3] = ROWCOL4(1,3);

	(*res)[2][0] = ROWCOL(2,0);
	(*res)[2][1] = ROWCOL(2,1);
	(*res)[2][2] = ROWCOL(2,2);
	(*res)[2][3] = ROWCOL4(2,3);

	(*res)[3][0] = ROWCOL(3,0);
	(*res)[3][1] = ROWCOL(3,1);
	(*res)[3][2] = ROWCOL(3,2);
	(*res)[3][3] = ROWCOL4(3,3);

	#undef ROWCOL
	#undef ROWCOL4
}
