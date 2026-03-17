import itertools
import operator

ops = [('+', operator.add), ('-', operator.sub), ('*', operator.mul), ('/', operator.truediv)]

def valid_results(nums, target=24, tol=1e-9):
    results = set()
    for nums_perm in set(itertools.permutations(nums)):
        a,b,c,d = nums_perm
        # all binary tree parenthesizations
        exprs = []
        # ((a op b) op c) op d
        exprs.append(("(({} {} {}) {} {}) {} {}", lambda op1,op2,op3: op3(op2(op1(a,b),c),d)))
        # ({} {} ({} {} {})) {} {}
        exprs.append(("({} {} ({} {} {})) {} {}", lambda op1,op2,op3: op3(op1(a,op2(b,c)),d)))
        # ({} {} {}) {} ({ } {} {})
        exprs.append(("({} {} {}) {} ({} {} {})", lambda op1,op2,op3: op2(op1(a,b),op3(c,d))))
        # {} {} (({} {} {}) {} {})
        exprs.append(("{} {} (({} {} {}) {} {})", lambda op1,op2,op3: op1(a,op3(op2(b,c),d))))
        # {} {} ({} {} ({} {} {}))
        exprs.append(("{} {} ({} {} ({} {} {}))", lambda op1,op2,op3: op1(a,op2(b,op3(c,d)))))

        for expr_fmt, compute_tree in exprs:
            for op1_sym, op1 in ops:
                for op2_sym, op2 in ops:
                    for op3_sym, op3 in ops:
                        try:
                            val = compute_tree(op1,op2,op3)
                        except ZeroDivisionError:
                            continue
                        if abs(val - target) < tol:
                            # build readable expression string by evaluating with floats carefully
                            # we'll construct according to the format chosen
                            # map ops to symbols in order
                            symbols = (op1_sym, op2_sym, op3_sym)
                            # create expression string by replacing placeholders sequentially
                            # simple approach: format based on which expr_fmt used
                            fmt = expr_fmt
                            expr_str = fmt.format(a, op1_sym, b, op2_sym, c, op3_sym, d)
                            results.add(expr_str)
  